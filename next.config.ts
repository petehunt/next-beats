import type { NextConfig } from 'next';

const nextConfig: NextConfig = {
  cacheComponents: true,
  cacheHandlers: {
    default: require.resolve('./cache-handler.cjs'),
  },
  experimental: {
    agentFeedback: true,
    inlineCss: true,
    useOffline: true,
  },
  async headers() {
    return [
      {
        headers: [{ key: 'Cache-Control', value: 'public, max-age=31536000, immutable' }],
        source: '/covers/:path*',
      },
    ];
  },
  outputFileTracingIncludes: {
    '/*': ['./crates/beats-node/*.node', './crates/beats-node/loader.cjs', './crates/beats-node/package.json'],
  },
  partialPrefetching: true,
  reactCompiler: true,
  serverExternalPackages: ['@next-beats/native'],
  turbopack: {
    rules: {
      '*.wgsl': {
        as: '*.js',
        loaders: ['@vgpu/wgsl/loader-webpack'],
      },
    },
  },
  typedRoutes: true,
};

export default nextConfig;
