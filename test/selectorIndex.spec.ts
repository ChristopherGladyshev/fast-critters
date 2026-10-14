import { parse } from 'css-what';
import { parseDocument } from 'htmlparser2';

import {
  buildCrittersClassCache,
  buildElementIndex,
  candidatesFor,
  compoundKeys,
  isSatisfiable,
  rightmostKeys,
} from '../src/core/selectorIndex';

import type { IDomNode, ISelectorToken } from '../src/core/types';

const tokens = (selector: string) => parse(selector)[0] as ISelectorToken[];

const htmlRoot = (html: string) => {
  const document = parseDocument(html) as unknown as IDomNode;
  return (document.children ?? []).find(
    (node) => (node as { name?: string }).name === 'html'
  ) as IDomNode;
};

describe('buildElementIndex', () => {
  const root = htmlRoot(
    '<html class="root"><head><style class="s">a{}</style><script id="js"></script></head>' +
      '<body><div class="a  b\n c a" id="x"><p class="b"></p><noscript><link class="n"></noscript></div>' +
      '<i id=""></i></body></html>'
  );
  const index = buildElementIndex(root);

  it('indexes descendants but not the root itself', () => {
    expect(index.byTag.has('html')).toBe(false);
    expect(index.byClass.has('root')).toBe(false);
    expect(index.byTag.get('body')).toHaveLength(1);
  });

  it('includes script and style elements', () => {
    expect(index.byTag.get('style')).toHaveLength(1);
    expect(index.byClass.get('s')).toHaveLength(1);
    expect(index.byId.get('js')).toHaveLength(1);
  });

  it('splits classes on any whitespace and lists an element once per class', () => {
    expect(index.byClass.get('a')).toHaveLength(1);
    expect(index.byClass.get('b')).toHaveLength(2);
    expect(index.byClass.get('c')).toHaveLength(1);
    expect(index.byClass.has('')).toBe(false);
  });

  it('keeps ids verbatim, including empty ones', () => {
    expect(index.byId.get('x')).toHaveLength(1);
    expect(index.byId.get('')).toHaveLength(1);
  });

  it('indexes elements inside noscript', () => {
    expect(index.byClass.get('n')).toHaveLength(1);
  });

  it('tolerates elements without attribs (created by critters)', () => {
    const parent = {
      type: 'tag',
      name: 'div',
      attribs: {},
      parent: null,
      children: [] as IDomNode[],
    };
    parent.children.push({
      type: 'style',
      name: 'style',
      parent,
      children: [],
    } as IDomNode);
    expect(
      buildElementIndex({
        type: 'root',
        parent: null,
        children: [parent],
      }).byTag.get('style')
    ).toHaveLength(1);
  });
});

describe('compoundKeys / rightmostKeys', () => {
  it('splits on every combinator', () => {
    expect(compoundKeys(tokens('DIV.a > p#x + span ~ em .b'))).toEqual([
      [
        { kind: 'tag', value: 'div' },
        { kind: 'class', value: 'a' },
      ],
      [
        { kind: 'tag', value: 'p' },
        { kind: 'id', value: 'x' },
      ],
      [{ kind: 'tag', value: 'span' }],
      [{ kind: 'tag', value: 'em' }],
      [{ kind: 'class', value: 'b' }],
    ]);
  });

  it('skips pseudo-class arguments, universal and non-indexable attributes', () => {
    expect(
      rightmostKeys(tokens('*.a:not(.b):is(.c)[data-x][class^=d][id|=e]'))
    ).toEqual([{ kind: 'class', value: 'a' }]);
  });

  it('treats [class~=x] as a class, but not with the i flag', () => {
    expect(rightmostKeys(tokens('[class~=x]'))).toEqual([
      { kind: 'class', value: 'x' },
    ]);
    expect(rightmostKeys(tokens('[class~=x i]'))).toEqual([]);
    expect(rightmostKeys(tokens('[id=y]'))).toEqual([
      { kind: 'id', value: 'y' },
    ]);
  });

  it('unescapes class names', () => {
    expect(rightmostKeys(tokens('.md\\:flex'))).toEqual([
      { kind: 'class', value: 'md:flex' },
    ]);
  });
});

describe('isSatisfiable / candidatesFor', () => {
  const root = htmlRoot(
    '<html class="root"><body><div class="a"><p class="b"></p><p></p></div></body></html>'
  );
  const index = buildElementIndex(root);

  it('needs every compound to exist somewhere', () => {
    expect(isSatisfiable(index, root, compoundKeys(tokens('.a .b')))).toBe(
      true
    );
    expect(
      isSatisfiable(index, root, compoundKeys(tokens('.missing .b')))
    ).toBe(false);
    expect(
      isSatisfiable(index, root, compoundKeys(tokens('.a .b.missing')))
    ).toBe(false);
  });

  it('counts the root as an ancestor', () => {
    expect(
      isSatisfiable(index, root, compoundKeys(tokens('html.root .b')))
    ).toBe(true);
    expect(
      isSatisfiable(index, root, compoundKeys(tokens('html.other .b')))
    ).toBe(false);
  });

  it('picks the smallest bucket, or null without keys', () => {
    expect(candidatesFor(index, rightmostKeys(tokens('p.b')))).toHaveLength(1);
    expect(candidatesFor(index, rightmostKeys(tokens('p')))).toHaveLength(2);
    expect(candidatesFor(index, rightmostKeys(tokens('p.zzz')))).toHaveLength(
      0
    );
    expect(candidatesFor(index, rightmostKeys(tokens('.a > *')))).toBeNull();
  });
});

describe('buildCrittersClassCache', () => {
  it('reproduces critters buildCache quirks', () => {
    const root = htmlRoot(
      '<html class="root"><head><style class="s"></style></head>' +
        '<body><div class="a  b\tc" id=" x "><script class="sc"></script></div></body></html>'
    );
    const cache = buildCrittersClassCache(root);
    expect([...cache.classes].sort()).toEqual(['', 'a', 'b\tc', 'root']);
    expect([...cache.ids]).toEqual(['x']);
  });
});
