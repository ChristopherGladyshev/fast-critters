import { readFileSync, realpathSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';

import { PACKAGE_NAME } from './core/native';

/** Опции critters, которые Next.js добавляет к своим (`...experimental.optimizeCss`). */
export type TCrittersOptions = Record<string, unknown>;

export interface IWithFastCrittersOptions {
  /** Объект для `experimental.optimizeCss`; по умолчанию `true` (опции Next.js). */
  critters?: TCrittersOptions;
  /** Каталог приложения, от которого резолвится `critters`; по умолчанию `process.cwd()`. */
  dir?: string;
}

// Next.js типизирует конфиг сам; здесь достаточно полей, которые меняем.
export type TNextConfig = {
  experimental?: Record<string, unknown>;
  outputFileTracingIncludes?: Record<string, string[]>;
  [key: string]: unknown;
};
export type TNextConfigFn = (
  ...args: unknown[]
) => TNextConfig | Promise<TNextConfig>;

const ALL_ROUTES = '/*';

/**
 * Где Next.js найдёт `critters`: он делает `require('critters')` из своего
 * рантайма страниц, одинаково при сборке webpack и Turbopack.
 */
function resolveCritters(dir: string): string | null {
  try {
    return dirname(require.resolve('critters/package.json', { paths: [dir] }));
  } catch {
    return null;
  }
}

function apply(config: TNextConfig, options: IWithFastCrittersOptions) {
  const dir = options.dir ?? process.cwd();
  const installed = resolveCritters(dir);
  const isPort =
    installed !== null &&
    JSON.parse(readFileSync(join(installed, 'package.json'), 'utf8')).name ===
      PACKAGE_NAME;
  if (!isPort) {
    console.warn(
      '[fast-critters] `critters` resolves to ' +
        (installed ?? 'nothing') +
        ', not to fast-critters: optimizeCss will use stock critters. ' +
        'Install it as `npm i critters@npm:fast-critters`.'
    );
  }

  const includes = { ...config.outputFileTracingIncludes };
  if (isPort) {
    // Аддон грузится по вычисленному пути, трассировка standalone его не видит.
    // require.resolve отдаёт realpath, поэтому и dir через realpath (на macOS /var → /private/var).
    const bin = `${relative(realpathSync(dir), installed as string).split('\\').join('/')}/bin/*.node`;
    includes[ALL_ROUTES] = [...(includes[ALL_ROUTES] ?? []), bin];
  }

  return {
    ...config,
    experimental: {
      ...config.experimental,
      optimizeCss: options.critters ?? config.experimental?.optimizeCss ?? true,
    },
    outputFileTracingIncludes: includes,
  };
}

/**
 * Включает `experimental.optimizeCss` на порте critters. Работает и с webpack,
 * и с Turbopack: критический CSS встраивается при рендере Pages Router.
 */
export function withFastCritters<T extends TNextConfig>(
  config?: T,
  options?: IWithFastCrittersOptions
): T & TNextConfig;
export function withFastCritters<T extends TNextConfigFn>(
  config: T,
  options?: IWithFastCrittersOptions
): (...args: unknown[]) => Promise<TNextConfig>;
export function withFastCritters(
  config: TNextConfig | TNextConfigFn = {},
  options: IWithFastCrittersOptions = {}
): TNextConfig | ((...args: unknown[]) => Promise<TNextConfig>) {
  if (typeof config === 'function') {
    return async (...args: unknown[]) => apply(await config(...args), options);
  }
  return apply(config, options);
}
