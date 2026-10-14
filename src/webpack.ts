import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { loadFastCritters } from './core/loadFastCritters';

import type { TCrittersConstructor } from './core/types';

export interface IFastCrittersPluginOptions {
  /** Опции critters поверх умолчаний (те же, что передаёт Next.js). */
  critters?: Record<string, unknown>;
  /** Какие HTML-файлы сборки обрабатывать; по умолчанию все `.html`. */
  test?: RegExp;
  /** Обрабатывать и в `mode: development` (по умолчанию только production). */
  isDevEnabled?: boolean;
}

// Минимум API webpack 5, который нужен плагину: без зависимости от его типов.
interface ICompiler {
  options: { mode?: string; output: { path?: string; publicPath?: unknown } };
  hooks: {
    afterEmit: {
      tapPromise(name: string, fn: (compilation: ICompilation) => Promise<void>): void;
    };
  };
}
interface ICompilation {
  getAssets(): { name: string }[];
  errors: Error[];
}

const NAME = 'FastCrittersPlugin';

/** Опции, с которыми Next.js создаёт critters: на них работает Rust-порт. */
const DEFAULTS = {
  ssrMode: true,
  reduceInlineStyles: false,
  preload: 'media',
  fonts: false,
};

const publicPathOf = (value: unknown) =>
  typeof value === 'string' && value !== 'auto' ? value : '/';

/**
 * Встраивает критический CSS в HTML, который выдала сборка webpack
 * (например, html-webpack-plugin). Файлы правятся на диске после emit:
 * CSS читается оттуда же, где его потом отдаст сервер.
 */
export class FastCrittersPlugin {
  private Critters: TCrittersConstructor | null = null;

  constructor(private readonly options: IFastCrittersPluginOptions = {}) {}

  apply(compiler: ICompiler): void {
    const { mode, output } = compiler.options;
    if (mode === 'development' && !this.options.isDevEnabled) {
      return;
    }
    const test = this.options.test ?? /\.html$/;

    compiler.hooks.afterEmit.tapPromise(NAME, async (compilation) => {
      const path = output.path as string;
      this.Critters ??= loadFastCritters();
      const critters = new this.Critters({
        ...DEFAULTS,
        path,
        publicPath: publicPathOf(output.publicPath),
        ...this.options.critters,
      });

      const files = compilation
        .getAssets()
        .map(({ name }) => name)
        .filter((name) => test.test(name));
      for (const name of files) {
        const file = join(path, name);
        try {
          const html = await readFile(file, 'utf8');
          await writeFile(file, await critters.process(html));
        } catch (error) {
          compilation.errors.push(
            new Error(`${NAME}: ${name}: ${(error as Error).message}`)
          );
        }
      }
    });
  }
}
