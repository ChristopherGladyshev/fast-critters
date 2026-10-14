import { empty } from './empty';

import {
  buildCrittersClassCache,
  buildElementIndex,
  candidatesFor,
  compoundKeys,
  isElement,
  isSatisfiable,
} from './selectorIndex';

import type {
  ICrittersClassCache,
  ICrittersContainer,
  ICrittersDocument,
  ICrittersStyle,
  IDomElement,
  IDomNode,
  IElementIndex,
  IFastCrittersDeps,
  ISelectorToken,
  TCompiledQuery,
  TCrittersConstructor,
  TExistsFn,
} from './types';

/*
 * critters держит кэш классов и id в глобалах модуля, поэтому при параллельных
 * рендерах второй документ, разобранный, пока первый ждёт стили, решает за
 * первый, какой CSS критический. Подкласс хранит кэш на документ.
 *
 * Он завязан на приватные методы critters (`embedLinkedStylesheet`,
 * `getAffectedStyleTags`, `processStyle`, `readFile`,
 * `document.crittersContainer`), поэтому версия critters закреплена
 * спеками эквивалентности.
 */

const MAX_CACHED_SELECTORS = 50_000;

/**
 * Короткий путь `cachedQuerySelector` из critters: первый однотокенный
 * подселектор по классу или id берётся из его кэша, какой бы ни была операция атрибута.
 */
const fromCrittersCache = (
  tokens: ISelectorToken[][],
  cache: ICrittersClassCache
) => {
  for (const subselector of tokens) {
    const [token] = subselector;
    if (
      subselector.length === 1 &&
      token.type === 'attribute' &&
      token.value !== undefined
    ) {
      if (token.name === 'class') {
        return cache.classes.has(token.value);
      }
      if (token.name === 'id') {
        return cache.ids.has(token.value);
      }
    }
  }
  return null;
};

const isWithin = (element: IDomNode, root: IDomNode) => {
  for (let node = element.parent; node; node = node.parent) {
    if (node === root) {
      return true;
    }
  }
  return false;
};

class BoundedCache<V> extends Map<string, V> {
  override set(key: string, value: V) {
    if (this.size >= MAX_CACHED_SELECTORS) {
      this.clear();
    }
    return super.set(key, value);
  }
}

class IndexedMatcher {
  private index: IElementIndex | null = null;
  private readonly memo = new Map<string, boolean>();
  private isMutated = false;

  constructor(
    private readonly container: ICrittersContainer,
    private readonly classCache: ICrittersClassCache,
    /** `false` для контейнера ниже <html>: css-select ограничивает им скомпилированные запросы. */
    private readonly isIndexed: boolean,
    private readonly original: TExistsFn,
    private readonly parse: (selector: string) => ISelectorToken[][] | null,
    private readonly compile: (selector: string) => TCompiledQuery | null
  ) {}

  /** Стиль ушёл из DOM: ответы для соседних и структурных селекторов могут измениться. */
  invalidate() {
    this.memo.clear();
    this.isMutated = true;
  }

  exists(selector: string): boolean {
    const known = this.memo.get(selector);
    if (known !== undefined) {
      return known;
    }

    let result: boolean | null;
    try {
      result = this.match(selector);
    } catch {
      // critters глотает исключения из exists() и выбрасывает правило: наши туда не пускаем.
      result = null;
    }

    // Оригинал может бросить на невалидном селекторе, critters это залогирует.
    result ??= this.original(selector);
    this.memo.set(selector, result);
    return result;
  }

  /** Ответ по индексу или `null`, если точно ответит только оригинал. */
  private match(selector: string): boolean | null {
    const tokens = this.parse(selector);
    if (!tokens) {
      return null;
    }

    const cached = fromCrittersCache(tokens, this.classCache);
    if (cached !== null) {
      return cached;
    }

    // `:scope` привязан к контексту поиска, а скомпилированный запрос его не несёт.
    if (!this.isIndexed || selector.includes('scope')) {
      return null;
    }

    this.index ??= buildElementIndex(this.container);
    const buckets: IDomElement[][] = [];
    for (const subselector of tokens) {
      const compounds = compoundKeys(subselector);
      if (!isSatisfiable(this.index, this.container, compounds)) {
        buckets.push([]);
        continue;
      }
      const candidates = candidatesFor(
        this.index,
        compounds[compounds.length - 1]
      );
      if (candidates === null) {
        return null;
      }
      buckets.push(candidates);
    }

    if (buckets.every((bucket) => empty(bucket))) {
      return false;
    }

    // Бросает так же, как css-select: пусть сообщит оригинал.
    const query = this.compile(selector);
    if (!query) {
      return null;
    }

    return buckets.some((bucket) =>
      bucket.some(
        (element) =>
          (!this.isMutated || isWithin(element, this.container)) &&
          query(element)
      )
    );
  }
}

export function createFastCritters<T extends TCrittersConstructor>(
  Critters: T,
  deps: IFastCrittersDeps
): T {
  const parsed = new BoundedCache<ISelectorToken[][] | null>();
  const compiled = new BoundedCache<TCompiledQuery | null>();
  const files = new BoundedCache<Promise<string>>();

  const parse = (selector: string) => {
    if (!parsed.has(selector)) {
      let tokens: ISelectorToken[][] | null;
      try {
        tokens = deps.parse(selector);
      } catch {
        tokens = null;
      }
      parsed.set(selector, tokens);
    }
    return parsed.get(selector) ?? null;
  };

  const compile = (selector: string) => {
    if (!compiled.has(selector)) {
      let query: TCompiledQuery | null;
      try {
        query = deps.compile(selector);
      } catch {
        query = null;
      }
      compiled.set(selector, query);
    }
    return compiled.get(selector) ?? null;
  };

  const matchers = new WeakMap<ICrittersDocument, IndexedMatcher>();
  const classCaches = new WeakMap<ICrittersDocument, ICrittersClassCache>();

  // Должно выполниться до того, как critters тронет DOM, как его buildCache() в createDocument().
  const snapshotClassCache = (document: ICrittersDocument) => {
    let cache = classCaches.get(document);
    if (!cache && document.crittersContainer) {
      cache = buildCrittersClassCache(document.crittersContainer);
      classCaches.set(document, cache);
    }
    return cache;
  };

  class FastCritters extends Critters {
    static readonly isFastCritters = true;

    override readFile(filename: string): Promise<string> {
      // Свой `fs` может отдать что угодно, неизменен в пределах сборки только диск.
      if (this.options.fs || deps.isFileCacheEnabled === false) {
        return super.readFile(filename);
      }

      let content = files.get(filename);
      if (!content) {
        content = super.readFile(filename);
        content.catch(() => files.delete(filename));
        files.set(filename, content);
      }
      return content;
    }

    override embedLinkedStylesheet(
      link: IDomElement,
      document: ICrittersDocument
    ) {
      // Первый вызов идёт синхронно из process(), сразу после createDocument().
      snapshotClassCache(document);
      return super.embedLinkedStylesheet(link, document);
    }

    override getAffectedStyleTags(document: ICrittersDocument) {
      const container = document.crittersContainer;
      const classCache = snapshotClassCache(document);
      if (container && classCache && !matchers.has(document)) {
        const original: TExistsFn = (selector) =>
          (Object.getPrototypeOf(container) as ICrittersContainer).exists.call(
            container,
            selector
          );
        const isIndexed = !(container.parent && isElement(container.parent));
        const matcher = new IndexedMatcher(
          container,
          classCache,
          isIndexed,
          original,
          parse,
          compile
        );
        matchers.set(document, matcher);
        Object.defineProperty(container, 'exists', {
          value: (selector: string) => matcher.exists(selector),
          configurable: true,
          writable: true,
        });
      }

      return super.getAffectedStyleTags(document);
    }

    override processStyle(style: ICrittersStyle, document: ICrittersDocument) {
      // processStyle у critters синхронен до return, поэтому выброшенный им стиль
      // уже отсоединён здесь, до сопоставления следующего.
      const done = super.processStyle(style, document);
      if (!style.parentNode) {
        matchers.get(document)?.invalidate();
      }
      return done;
    }
  }

  return FastCritters;
}
