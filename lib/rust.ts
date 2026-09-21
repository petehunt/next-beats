import { createRequire } from 'node:module';
import type * as NativeBinding from '@next-beats/native';

// Keep the native addon outside Next's server bundles. Node resolves it from the
// workspace at runtime, which works in dev, next start, and traced deployments.
const requireFromWorkspace = createRequire(`${process.cwd()}/package.json`);

export const rust = requireFromWorkspace('@next-beats/native') as typeof NativeBinding;

