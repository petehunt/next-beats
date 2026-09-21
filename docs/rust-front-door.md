# Rust front-door architecture

The application has one domain model in `beats-core` and three runtime adapters:

| Adapter | Purpose |
| --- | --- |
| `beats-front` | Public HTTP ingress. Handles Rust-native routes and streams every other request to a Next origin. |
| `beats-node` | N-API bridge for code that runs inside `next dev` or `next start`. |
| Next route/proxy files | Thin framework adapters for Next request objects and cache APIs. |

## Request ownership

The front door currently handles these requests without entering the Next request stack:

- `GET /_rust/health`
- `POST /api/play` (auth, validation, transaction, and cache-tag derivation)
- `/covers/*` and `/logo.svg`
- anonymous page redirects to `/login`

Everything else is streamed to `BEATS_NEXT_ORIGIN`. The default is
`http://127.0.0.1:3001`, used by both `pnpm dev` and `pnpm start`. The proxy
preserves methods, bodies, response streaming, cookies, and forwarding headers.

The play mutation derives its Next cache tags in Rust. When the Rust ingress
owns the request, it posts those tags to the protected
`/api/_rust/revalidate` Next adapter. When Next owns the request directly,
`app/api/play/route.ts` gets the same tags over N-API and applies them locally.
Only the call to `revalidateTag` is expressed in TypeScript because it is a
Next runtime capability.

## Runtime modes

- `pnpm dev`: Rust listens publicly on port 3000; `next dev` listens on 3001.
- `pnpm start`: Rust listens publicly on port 3000; `next start` listens on 3001.
- `pnpm dev:next` / `pnpm start:next`: run Next directly. Proxy and play still
  execute the shared Rust core through N-API.
- Remote origin: set `BEATS_NEXT_ORIGIN` to an HTTPS Next deployment. This lets
  the same Rust ingress sit in front of a Vercel deployment without assuming a
  co-located Node process.

Set `BEATS_INTERNAL_TOKEN` to the same high-entropy value in the Rust and Next
environments outside local development. The Rust ingress never forwards the
internal cache endpoint from the public network.

## Vercel and minimal mode

`minimalMode` is a property of platform-invoked Next entrypoints, not a safer
or smaller replacement for `next start`. The Rust server therefore does not set
`NEXT_MINIMAL`. In the near-term Vercel topology, Vercel remains the Next origin
and performs its own build-output routing; Rust forwards fallback requests to
the deployment URL.

The next step toward a fully native Vercel-style runtime is a Next Deployment
Adapter (`adapterPath`) that emits a compact routing manifest for
`beats-front`. That manifest will let Rust:

1. serve `STATIC_FILE` and complete `PRERENDER` outputs itself;
2. apply redirects, rewrites, headers, and route matching before compute;
3. invoke only the selected `APP_PAGE` or `APP_ROUTE` minimal-mode entrypoint,
   supplying the matched-path and PPR resume headers described by Next's
   adapter API;
4. replace the revalidation HTTP adapter with shared `cacheHandler` /
   `cacheHandlers` tag state when deployments become multi-instance.

The `BEATS_NEXT_ORIGIN` boundary is intentionally narrow so that origin proxying
can be replaced by that entrypoint invoker without changing Rust-owned routes
or domain logic.

