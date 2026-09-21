import { NextResponse, type NextRequest } from 'next/server';
import { rust } from '@/lib/rust';

export function proxy(request: NextRequest) {
  const decision = rust.proxyDecision(
    request.nextUrl.pathname,
    request.headers.get('cookie') ?? undefined,
  );
  if (decision.redirectPathname) {
    const url = request.nextUrl.clone();
    url.pathname = decision.redirectPathname;
    return NextResponse.redirect(url);
  }

  return NextResponse.next();
}

export const config = {
  matcher: ['/((?!_next|api|icon|favicon).*)'],
};
