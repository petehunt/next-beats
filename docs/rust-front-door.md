# Rust runtime architecture

Business logic lives once in `beats-core`. Three small adapters expose it:

| Runtime           | Adapter            | Requests                                                                                           |
| ----------------- | ------------------ | -------------------------------------------------------------------------------------------------- |
| Local/self-hosted | `beats-front`      | Rust owns auth redirects, static covers, health, and `/api/play`; other traffic streams to Next.   |
| Direct Next       | `beats-node`       | `proxy.ts` and the play route call the same crate through N-API.                                   |
| Vercel            | `api/rust-play.rs` | Vercel's Rust runtime owns `/api/play`; Vercel's router and Next runtime own pages and prerenders. |

## Local and self-hosted

`pnpm dev` and `pnpm start` expose the Rust front door on port 3000 and run Next
on port 3001. `BEATS_NEXT_ORIGIN` can point the front door at another Next
origin. `pnpm dev:next` and `pnpm start:next` remain available when Next needs to
run directly.

The N-API package commits only a small platform loader and its public types.
`pnpm native:build` generates the platform `.node` binary locally. Next's file
tracing configuration explicitly includes that binary in production functions.

## Cache invalidation

The play transaction derives its tags in Rust and atomically upserts them into
`RustCacheTag` before commit. Next's custom cache handler keeps cache values in a
bounded in-process LRU and synchronizes tag timestamps from PostgreSQL at the
start of each cache-using request. Node mutations use the same handler's
`updateTags` method, so every runtime shares one invalidation mechanism.

There is no cache-invalidation callback route and no second Node request after a
Rust mutation.

## Vercel

Vercel's official Rust Functions runtime compiles `api/rust-play.rs`.
`vercel.json` rewrites the public `/api/play` path to that function, so the
mutation never enters a Node function. Static assets and complete prerenders are
served by Vercel's router/CDN. Remaining Next routes use Vercel's platform-owned
minimal-mode entrypoints.

The application deliberately does not set `NEXT_MINIMAL` or reimplement Next's
private routing protocol. Minimal mode is the interface between Next build
outputs and Vercel's runtime; application code should not impersonate it.
