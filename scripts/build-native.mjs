// Собирает Rust-аддон (native/) и кладёт его в bin/ под именем текущей
// платформы: bin/fast-critters.<platform>-<arch>[-gnu|-musl].node.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const native = join(root, 'native');
const isDebug = process.argv.includes('--debug');

execFileSync(
  'cargo',
  ['build', '-p', 'critters-rs', ...(isDebug ? [] : ['--release'])],
  { cwd: native, stdio: 'inherit' }
);

const lib = {
  linux: 'libcritters_rs.so',
  darwin: 'libcritters_rs.dylib',
  win32: 'critters_rs.dll',
}[process.platform];
if (!lib) {
  throw new Error(`unsupported platform: ${process.platform}`);
}

const { nativeBinaryName } = createRequire(import.meta.url)(
  join(root, 'dist/core/native.js')
);
const target = join(root, 'bin', nativeBinaryName());
mkdirSync(dirname(target), { recursive: true });
copyFileSync(
  join(native, 'target', isDebug ? 'debug' : 'release', lib),
  target
);
console.log(`native addon: ${target}`);
