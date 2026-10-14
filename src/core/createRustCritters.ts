import { empty } from './empty';

import type {
  ICrittersInstance,
  INativeCritters,
  INativeOptions,
  TCrittersConstructor,
} from './types';

const NS_PER_MS = 1_000_000;

const isEmptyOrUnset = (value: unknown) =>
  Array.isArray(value) ? empty(value) : value === undefined;

/** Значения опций, которые реализует Rust-порт: ровно то, что передаёт Next.js. */
const SUPPORTED: Record<string, (value: unknown) => boolean> = {
  ssrMode: () => true,
  logLevel: () => true,
  logger: () => true,
  path: (value) => typeof value === 'string',
  publicPath: (value) => typeof value === 'string',
  preload: (value) => value === 'media',
  fonts: (value) => value === false,
  reduceInlineStyles: (value) => value === false,
  pruneSource: (value) => value === undefined || value === false,
  external: (value) => value === undefined || value === true,
  mergeStylesheets: (value) => value === undefined || value === true,
  compress: (value) => value === undefined || value === true,
  noscriptFallback: (value) => value === undefined || value === true,
  keyframes: (value) => value === undefined || value === 'critical',
  additionalStylesheets: isEmptyOrUnset,
  allowRules: isEmptyOrUnset,
};

const nativeOptions = (instance: ICrittersInstance): INativeOptions | null => {
  const { options } = instance;
  if (instance.fs !== undefined) {
    return null;
  }
  for (const [key, value] of Object.entries(options)) {
    const supported = SUPPORTED[key];
    if (supported ? !supported(value) : value !== undefined) {
      return null;
    }
  }
  return {
    path: options.path as string,
    publicPath: options.publicPath as string,
  };
};

/** Вывод совпадает с critters байт в байт, остальное уходит в `Base`. */
export function createRustCritters<T extends TCrittersConstructor>(
  Base: T,
  native: INativeCritters
): T {
  class RustCritters extends Base {
    static readonly isRustCritters = true;

    override async process(html: string): Promise<string> {
      const options = nativeOptions(this);
      if (!options) {
        return super.process(html);
      }

      const start = process.hrtime.bigint();
      let result;
      try {
        result = await native.process(html, options);
      } catch (error) {
        this.logger.warn(
          `critters-rs failed, using JS: ${(error as Error).message}`
        );
        return super.process(html);
      }
      if (typeof result.html !== 'string') {
        this.logger.warn(`critters-rs fallback to JS: ${result.fallback}`);
        return super.process(html);
      }

      for (const { level, message } of result.logs) {
        this.logger[level === 'warn' ? 'warn' : 'info'](message);
      }
      const end = process.hrtime.bigint();
      this.logger.info('Time ' + Number(end - start) / NS_PER_MS);
      return result.html;
    }
  }

  return RustCritters;
}
