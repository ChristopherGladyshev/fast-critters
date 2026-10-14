import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

export const PACKAGE_NAME = 'fast-critters';

/** Суффикс libc для linux: прод-образы бывают и на glibc, и на musl (alpine). */
const libc = (): string => {
  if (process.platform !== 'linux') {
    return '';
  }
  const report = process.report?.getReport() as
    | { header?: { glibcVersionRuntime?: string } }
    | undefined;
  return report?.header?.glibcVersionRuntime ? '-gnu' : '-musl';
};

/** Имя бинарника аддона под текущую платформу: `fast-critters.linux-x64-gnu.node`. */
export const nativeBinaryName = (): string =>
  `fast-critters.${process.platform}-${process.arch}${libc()}.node`;

/** Корень пакета: из `dist/` и из `src/core/` путь разный. */
export function packageRoot(from = __dirname): string {
  for (let dir = from; ; dir = dirname(dir)) {
    const manifest = join(dir, 'package.json');
    if (
      existsSync(manifest) &&
      JSON.parse(readFileSync(manifest, 'utf8')).name === PACKAGE_NAME
    ) {
      return dir;
    }
    if (dirname(dir) === dir) {
      return join(__dirname, '..');
    }
  }
}

/** Собранный аддон из `bin/` пакета или null, если под эту платформу его нет. */
export function findBundledBinary(): string | null {
  const path = join(packageRoot(), 'bin', nativeBinaryName());
  return existsSync(path) ? path : null;
}
