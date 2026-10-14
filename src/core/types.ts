/** Минимальный интерфейс логгера: совместим с console и pino. */
export interface ILogger {
  info(message: string): void;
  warn(message: string): void;
}


export type TExistsFn = (selector: string) => boolean;
export type TCompiledQuery = (element: IDomElement) => boolean;

export interface ICrittersContainer extends IDomElement {
  exists: TExistsFn;
}

export interface ICrittersDocument extends IDomNode {
  crittersContainer?: ICrittersContainer;
}

export interface ICrittersStyle extends IDomElement {
  parentNode: IDomNode | null;
}

export interface ICrittersInstance {
  options: Record<string, unknown>;
  logger: ILogger;
  fs?: unknown;
  readFile(filename: string): Promise<string>;
  embedLinkedStylesheet(
    link: IDomElement,
    document: ICrittersDocument
  ): Promise<unknown>;
  getAffectedStyleTags(document: ICrittersDocument): ICrittersStyle[];
  processStyle(
    style: ICrittersStyle,
    document: ICrittersDocument
  ): Promise<void>;
  process(html: string): Promise<string>;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type TCrittersConstructor = new (...args: any[]) => ICrittersInstance;

export interface IFastCrittersDeps {
  /** `parse` из css-what: копию этого парсера critters использует в `cachedQuerySelector`. */
  parse(selector: string): ISelectorToken[][];
  /** `compile` из css-select: тот же движок, что в `selectOne` у critters. */
  compile(selector: string): TCompiledQuery;
  /** Держать CSS-файлы в памяти между документами (по умолчанию true). */
  isFileCacheEnabled?: boolean;
}

export interface ILoadedCritters {
  Critters: TCrittersConstructor;
  deps: Pick<IFastCrittersDeps, 'parse' | 'compile'>;
}

export interface ICrittersSetup extends ILoadedCritters {
  native: INativeCritters | null;
  isInlineAllCss: boolean;
}

/** napi-аддон из native/ (Rust-порт critters). */
export interface INativeCritters {
  process(html: string, options: INativeOptions): Promise<INativeResult>;
}

export interface INativeOptions {
  path: string;
  publicPath: string;
}

export interface INativeResult {
  /** Нет, если документ должна обработать JS-реализация. */
  html?: string | null;
  logs: { level: string; message: string }[];
  fallback?: string | null;
}

/** Переменные окружения, из которых читаются флаги critters. */
export type TCrittersEnv = Record<string, string | undefined>;

export type TCrittersLog = ILogger;

export interface IDomNode {
  type: string;
  parent: IDomNode | null;
  children?: IDomNode[];
}

export interface IDomElement extends IDomNode {
  name: string;
  // Нет у элементов, которые critters создаёт сам (`document.createElement`).
  attribs?: Record<string, string>;
}

export interface ISelectorToken {
  type: string;
  name?: string;
  action?: string;
  value?: string;
  ignoreCase?: boolean | 'quirks' | null;
}

export interface IElementIndex {
  byTag: Map<string, IDomElement[]>;
  byClass: Map<string, IDomElement[]>;
  byId: Map<string, IDomElement[]>;
}

export type TSelectorKey = { kind: 'tag' | 'class' | 'id'; value: string };

export interface ICrittersClassCache {
  classes: Set<string>;
  ids: Set<string>;
}
