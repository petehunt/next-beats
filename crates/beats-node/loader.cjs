const path = require('node:path');

function platformTarget() {
  if (process.platform === 'darwin') return `darwin-${process.arch}`;
  if (process.platform === 'win32') return `win32-${process.arch}-msvc`;
  if (process.platform === 'linux') {
    const glibc = process.report?.getReport?.().header?.glibcVersionRuntime;
    return `linux-${process.arch}-${glibc ? 'gnu' : 'musl'}`;
  }
  throw new Error(`Unsupported native platform: ${process.platform}-${process.arch}`);
}

module.exports = require(path.join(__dirname, `beats-native.${platformTarget()}.node`));
