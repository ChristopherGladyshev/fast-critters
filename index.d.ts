import type { ICrittersInstance } from './dist/core/index';

/** Класс, совместимый с `critters` 0.0.23: `new Critters(options).process(html)`. */
declare const Critters: new (options?: Record<string, unknown>) => ICrittersInstance;
export = Critters;
