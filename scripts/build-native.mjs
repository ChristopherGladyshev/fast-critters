// Собирает Rust-аддон (native/) и кладёт его в bin/ под именем платформы:
// bin/fast-critters.<platform>-<arch>[-gnu|-musl].node.
//
// Без аргументов — под текущую машину. Для CI:
//   --target <triple>  кросс-сборка (имя берётся из TARGETS);
//   --zig              через cargo-zigbuild (linux: glibc 2.17 и musl).
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const native = join(root, 'native');
const args = process.argv.slice(2);
const isDebug = args.includes('--debug');
const useZig = args.includes('--zig');
const target = args.includes('--target')
  ? args[args.indexOf('--target') + 1]
  : null;

/** Rust target → суффикс имени бинарника (как в nativeBinaryName). */
const TARGETS = {
  'x86_64-unknown-linux-gnu': 'linux-x64-gnu',
  'aarch64-unknown-linux-gnu': 'linux-arm64-gnu',
  'x86_64-unknown-linux-musl': 'linux-x64-musl',
  'aarch64-unknown-linux-musl': 'linux-arm64-musl',
  'x86_64-apple-darwin': 'darwin-x64',
  'aarch64-apple-darwin': 'darwin-arm64',
  'x86_64-pc-windows-msvc': 'win32-x64',
  'aarch64-pc-windows-msvc': 'win32-arm64',
};

if (target && !TARGETS[target]) {
  throw new Error(`unknown target: ${target}`);
}

const isMusl = target?.endsWith('-musl');
// Минимальный glibc для gnu-сборок через zig: работает на старых дистрибутивах.
const cargoTarget =
  useZig && target?.endsWith('-linux-gnu') ? `${target}.2.17` : target;

execFileSync(
  'cargo',
  [
    useZig ? 'zigbuild' : 'build',
    '-p',
    'critters-rs',
    ...(isDebug ? [] : ['--release']),
    ...(cargoTarget ? ['--target', cargoTarget] : []),
  ],
  {
    cwd: native,
    stdio: 'inherit',
    // cdylib под musl требует динамической libc.
    env: isMusl
      ? {
          ...process.env,
          RUSTFLAGS: `${process.env.RUSTFLAGS ?? ''} -C target-feature=-crt-static`.trim(),
        }
      : process.env,
  }
);

const os = target
  ? target.includes('-linux-')
    ? 'linux'
    : target.includes('-apple-')
      ? 'darwin'
      : 'win32'
  : process.platform;
const lib = {
  linux: 'libcritters_rs.so',
  darwin: 'libcritters_rs.dylib',
  win32: 'critters_rs.dll',
}[os];
if (!lib) {
  throw new Error(`unsupported platform: ${os}`);
}

const name = target
  ? `fast-critters.${TARGETS[target]}.node`
  : createRequire(import.meta.url)(
      join(root, 'dist/core/native.js')
    ).nativeBinaryName();
const out = join(root, 'bin', name);
mkdirSync(dirname(out), { recursive: true });
copyFileSync(
  join(native, 'target', target ?? '', isDebug ? 'debug' : 'release', lib),
  out
);
console.log(`native addon: ${out}`);
