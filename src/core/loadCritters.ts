import { createRequire } from 'node:module';

import type {
  ILoadedCritters,
  ISelectorToken,
  TCompiledQuery,
  TCrittersConstructor,
} from './types';

/** Оригинальный critters ставится под этим именем, `critters` — порт (libs/critters-fast). */
export const CRITTERS_UPSTREAM = 'critters-upstream';

/**
 * Загружает critters вместе с css-select и css-what, которые он находит сам:
 * быстрый путь компилирует селекторы тем же движком, что и оригинал.
 */
export function loadCritters(): ILoadedCritters {
  const path = createRequire(__filename).resolve(CRITTERS_UPSTREAM);
  const crittersRequire = createRequire(path);
  const cssSelect = crittersRequire('css-select') as {
    compile(selector: string): TCompiledQuery;
  };
  const cssWhat = createRequire(crittersRequire.resolve('css-select'))(
    'css-what'
  ) as { parse(selector: string): ISelectorToken[][] };

  return {
    Critters: crittersRequire(path) as TCrittersConstructor,
    deps: {
      parse: (selector) => cssWhat.parse(selector),
      compile: (selector) => cssSelect.compile(selector),
    },
  };
}
