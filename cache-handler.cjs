const { Pool } = require('pg');

const MAX_CACHE_BYTES = 50 * 1024 * 1024;
const cache = new Map();
const pendingSets = new Map();
const tagState = new Map();
const pool = new Pool({ connectionString: process.env.DATABASE_URL });
let cacheBytes = 0;

function streamFromBytes(bytes) {
  return new ReadableStream({
    start(controller) {
      controller.enqueue(bytes);
      controller.close();
    },
  });
}

function touch(cacheKey, entry) {
  cache.delete(cacheKey);
  cache.set(cacheKey, entry);
}

function store(cacheKey, entry) {
  const previous = cache.get(cacheKey);
  if (previous) cacheBytes -= previous.value.byteLength;
  cache.set(cacheKey, entry);
  cacheBytes += entry.value.byteLength;

  while (cacheBytes > MAX_CACHE_BYTES && cache.size > 1) {
    const oldestKey = cache.keys().next().value;
    const oldest = cache.get(oldestKey);
    cache.delete(oldestKey);
    cacheBytes -= oldest.value.byteLength;
  }
}

function invalidationFor(tags, timestamp) {
  let newest;
  for (const tag of tags) {
    const state = tagState.get(tag);
    if (state && state.revalidatedAt > timestamp) {
      if (!newest || state.revalidatedAt > newest.revalidatedAt) newest = state;
    }
  }
  return newest;
}

module.exports = {
  async get(cacheKey, softTags) {
    const pending = pendingSets.get(cacheKey);
    if (pending) await pending;

    const entry = cache.get(cacheKey);
    if (!entry) return undefined;
    const invalidation = invalidationFor([...entry.tags, ...softTags], entry.timestamp);
    if (invalidation?.expireSeconds === 0) return undefined;

    touch(cacheKey, entry);
    return {
      ...entry,
      revalidate: invalidation ? -1 : entry.revalidate,
      value: streamFromBytes(entry.value),
    };
  },

  async getExpiration() {
    // Next passes soft tags to get() when this returns Infinity. That lets one
    // code path apply the shared state to both explicit and implicit tags.
    return Infinity;
  },

  async refreshTags() {
    try {
      const result = await pool.query('SELECT "tag", "revalidatedAt", "expireSeconds" FROM "RustCacheTag"');
      for (const row of result.rows) {
        const sharedTimestamp = Number(row.revalidatedAt);
        const previous = tagState.get(row.tag);
        if (previous?.sharedTimestamp === sharedTimestamp && previous.expireSeconds === row.expireSeconds) continue;
        tagState.set(row.tag, {
          expireSeconds: row.expireSeconds,
          // Timestamp the moment this process observes the committed change.
          // This also invalidates a local render that overlapped the commit.
          revalidatedAt: Date.now(),
          sharedTimestamp,
        });
      }
    } catch (error) {
      console.error('Unable to refresh shared cache tags', error);
    }
  },

  async set(cacheKey, pendingEntry) {
    const operation = (async () => {
      const entry = await pendingEntry;
      const value = new Uint8Array(await new Response(entry.value).arrayBuffer());
      store(cacheKey, { ...entry, value });
    })();
    pendingSets.set(cacheKey, operation);
    try {
      await operation;
    } finally {
      if (pendingSets.get(cacheKey) === operation) pendingSets.delete(cacheKey);
    }
  },

  async updateTags(tags, durations) {
    if (tags.length === 0) return;
    const expireSeconds = durations?.expire ?? 0;
    const expirations = tags.map(() => expireSeconds);
    const result = await pool.query(
      `
        WITH event AS (
          SELECT (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::bigint AS "revalidatedAt"
        )
        INSERT INTO "RustCacheTag" ("tag", "revalidatedAt", "expireSeconds")
        SELECT tag, event."revalidatedAt", "expireSeconds"
        FROM UNNEST($1::text[], $2::int[]) AS input(tag, "expireSeconds")
        CROSS JOIN event
        ON CONFLICT ("tag") DO UPDATE SET
          "revalidatedAt" = GREATEST(
            "RustCacheTag"."revalidatedAt",
            EXCLUDED."revalidatedAt"
          ),
          "expireSeconds" = CASE
            WHEN EXCLUDED."revalidatedAt" >= "RustCacheTag"."revalidatedAt"
            THEN EXCLUDED."expireSeconds"
            ELSE "RustCacheTag"."expireSeconds"
          END
        RETURNING "tag", "revalidatedAt", "expireSeconds"
      `,
      [tags, expirations],
    );
    const observedAt = Date.now();
    for (const row of result.rows) {
      tagState.set(row.tag, {
        expireSeconds: row.expireSeconds,
        revalidatedAt: observedAt,
        sharedTimestamp: Number(row.revalidatedAt),
      });
    }
  },
};
