import { empty } from './empty';

import type {
  ICrittersClassCache,
  IDomElement,
  IDomNode,
  IElementIndex,
  ISelectorToken,
  TSelectorKey,
} from './types';

/*
 * Элемент, подходящий под селектор, несёт все простые селекторы его правого
 * компаунда, поэтому лежит в корзине любого из них: проверка одной корзины
 * вместо всего дерева даёт тот же ответ.
 */

// Как `isTag` в domutils: script и style тоже элементы.
const ELEMENT_TYPES = new Set(['tag', 'script', 'style']);

const TRAVERSAL_TYPES = new Set([
  'descendant',
  'child',
  'sibling',
  'adjacent',
  'parent',
  'column-combinator',
]);

// css-select сверяет `.a` с классами, разделёнными любыми пробельными символами.
const WHITESPACE = /\s+/;

export const isElement = (node: IDomNode): node is IDomElement =>
  ELEMENT_TYPES.has(node.type);

const push = (
  map: Map<string, IDomElement[]>,
  key: string,
  el: IDomElement
) => {
  const bucket = map.get(key);
  if (bucket) {
    bucket.push(el);
  } else {
    map.set(key, [el]);
  }
};

/** Индексирует потомков `root`, но не сам `root`: так css-select задаёт контекст поиска. */
export function buildElementIndex(root: IDomNode): IElementIndex {
  const index: IElementIndex = {
    byTag: new Map(),
    byClass: new Map(),
    byId: new Map(),
  };
  const stack: IDomNode[] = [...(root.children ?? [])].reverse();

  while (stack.length > 0) {
    const node = stack.pop() as IDomNode;
    if (!isElement(node)) {
      continue;
    }

    push(index.byTag, node.name, node);

    const className = node.attribs?.class;
    if (className) {
      const seen = new Set<string>();
      for (const cls of className.split(WHITESPACE)) {
        if (cls && !seen.has(cls)) {
          seen.add(cls);
          push(index.byClass, cls, node);
        }
      }
    }

    const id = node.attribs?.id;
    if (id !== undefined) {
      push(index.byId, id, node);
    }

    const children = node.children ?? [];
    for (let i = children.length - 1; i >= 0; i--) {
      stack.push(children[i]);
    }
  }

  return index;
}

const keyOf = (token: ISelectorToken): TSelectorKey | null => {
  if (token.type === 'tag' && token.name && token.name !== '*') {
    return { kind: 'tag', value: token.name.toLowerCase() };
  }
  if (
    token.type !== 'attribute' ||
    token.ignoreCase === true ||
    token.value === undefined
  ) {
    return null;
  }
  if (token.name === 'class' && token.action === 'element') {
    return { kind: 'class', value: token.value };
  }
  if (token.name === 'id' && token.action === 'equals') {
    return { kind: 'id', value: token.value };
  }
  return null;
};

/**
 * Индексируемые простые селекторы каждого компаунда слева направо. Аргументы
 * псевдоклассов (`:not(.a)`, `:is(.b)`) не обязательны для совпадения и пропускаются.
 */
export function compoundKeys(selector: ISelectorToken[]): TSelectorKey[][] {
  const compounds: TSelectorKey[][] = [[]];
  for (const token of selector) {
    if (TRAVERSAL_TYPES.has(token.type)) {
      compounds.push([]);
      continue;
    }
    const key = keyOf(token);
    if (key) {
      compounds[compounds.length - 1].push(key);
    }
  }
  return compounds;
}

/** Простые селекторы правого компаунда, которые есть у каждого подходящего элемента. */
export function rightmostKeys(selector: ISelectorToken[]): TSelectorKey[] {
  const compounds = compoundKeys(selector);
  return compounds[compounds.length - 1];
}

const bucketOf = (index: IElementIndex, key: TSelectorKey) =>
  (key.kind === 'tag'
    ? index.byTag
    : key.kind === 'class'
      ? index.byClass
      : index.byId
  ).get(key.value);

const rootHasKey = (root: IDomNode, key: TSelectorKey) => {
  if (!isElement(root)) {
    return false;
  }
  if (key.kind === 'tag') {
    return root.name === key.value;
  }
  if (key.kind === 'id') {
    return root.attribs?.id === key.value;
  }
  return (root.attribs?.class ?? '').split(WHITESPACE).includes(key.value);
};

/**
 * `false`, если какому-то компаунду нужен класс, id или тег, которого нет ни у
 * одного элемента: ни у потомков из индекса, ни у `root` (предка в `html .a`).
 */
export function isSatisfiable(
  index: IElementIndex,
  root: IDomNode,
  compounds: TSelectorKey[][]
) {
  return compounds.every((keys) =>
    keys.every(
      (key) => (bucketOf(index, key)?.length ?? 0) > 0 || rootHasKey(root, key)
    )
  );
}

const EMPTY: IDomElement[] = [];

/** Самая маленькая корзина по ключам или `null`, если у компаунда нет ключа для индекса. */
export function candidatesFor(
  index: IElementIndex,
  keys: TSelectorKey[]
): IDomElement[] | null {
  if (empty(keys)) {
    return null;
  }

  let best: IDomElement[] | null = null;
  for (const key of keys) {
    const bucket = bucketOf(index, key) ?? EMPTY;
    if (best === null || bucket.length < best.length) {
      best = bucket;
    }
    if (empty(best)) {
      break;
    }
  }

  return best;
}

/**
 * Копия `buildCache()` из critters на один документ, вместе с его странностями:
 * классы делятся по одиночному пробелу, id обрезаются, поддеревья script/style
 * пропускаются, сам контейнер учитывается. critters держит этот кэш в глобалах
 * модуля, и параллельный рендер перезаписывает его, пока первый ждёт стили.
 */
export function buildCrittersClassCache(
  container: IDomNode
): ICrittersClassCache {
  const cache: ICrittersClassCache = { classes: new Set(), ids: new Set() };
  const queue: IDomNode[] = [container];

  for (let i = 0; i < queue.length; i++) {
    const node = queue[i] as IDomElement;
    const className = node.attribs?.class;
    if (className != null) {
      for (const cls of className.trim().split(' ')) {
        cache.classes.add(cls);
      }
    }
    const id = node.attribs?.id;
    if (id != null) {
      cache.ids.add(id.trim());
    }
    for (const child of node.children ?? []) {
      if (child.type === 'tag') {
        queue.push(child);
      }
    }
  }

  return cache;
}
