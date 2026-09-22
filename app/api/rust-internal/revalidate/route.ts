import { revalidateTag } from 'next/cache';

const LOCAL_INTERNAL_TOKEN = 'local-development-only';

export async function POST(request: Request) {
  const token =
    process.env.BEATS_INTERNAL_TOKEN ?? (process.env.NODE_ENV === 'development' ? LOCAL_INTERNAL_TOKEN : undefined);
  if (!token || request.headers.get('x-beats-internal-token') !== token) {
    return new Response(null, { status: 404 });
  }

  const body: unknown = await request.json().catch(() => null);
  if (!isTagRequest(body)) return new Response(null, { status: 400 });

  for (const tag of body.tags) revalidateTag(tag, 'max');
  return new Response(null, { status: 204 });
}

function isTagRequest(value: unknown): value is { tags: string[] } {
  return (
    typeof value === 'object' &&
    value !== null &&
    'tags' in value &&
    Array.isArray(value.tags) &&
    value.tags.every(tag => typeof tag === 'string' && tag.length <= 256)
  );
}
