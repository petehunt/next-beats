import { rust } from '@/lib/rust';
import type { NextRequest } from 'next/server';

export async function POST(request: NextRequest) {
  const result = await rust.handlePlay(
    process.env.DATABASE_URL!,
    request.headers.get('cookie') ?? undefined,
    await request.text(),
  );
  return new Response(null, { status: result.status });
}
