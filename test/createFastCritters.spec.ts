import fs from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { createFastCritters } from '../src/core/createFastCritters';
import { loadCritters } from '../src/core/loadCritters';

const { Critters, deps } = loadCritters();

// Опции, которые Next.js передаёт при `experimental.optimizeCss` (next/dist/server/post-process.js).
const nextOptions = (path: string) => ({
  ssrMode: true,
  reduceInlineStyles: false,
  path,
  publicPath: '/_next/',
  preload: 'media',
  fonts: false,
  logLevel: 'silent',
});

const dir = fs.mkdtempSync(join(tmpdir(), 'fast-critters-'));
afterAll(() => fs.rmSync(dir, { recursive: true, force: true }));

let sheetSeq = 0;
const writeSheet = (css: string) => {
  const name = `s${sheetSeq++}.css`;
  fs.writeFileSync(join(dir, name), css);
  return `/_next/${name}`;
};

const page = (body: string, sheets: string[], head = '') =>
  `<!DOCTYPE html><html lang="ru" class="page-root"><head>${head}${sheets
    .map((href) => `<link rel="stylesheet" href="${href}" data-n-g=""/>`)
    .join('')}</head><body>${body}</body></html>`;

const FastCritters = createFastCritters(Critters, deps);

const both = async (html: string, options = {}) => {
  const original = await new Critters({
    ...nextOptions(dir),
    ...options,
  }).process(html);
  const fast = await new FastCritters({
    ...nextOptions(dir),
    ...options,
  }).process(html);
  return { original, fast };
};

const BODY =
  '<header class="Header_root__a1 js-sticky"><nav id="main-nav" class="nav"><a class="nav__link is-active" href="/">x</a>' +
  '<a class="nav__link" href="/b">y</a></nav></header>' +
  '<main><ul class="list"><li class="list__item">1</li><li class="list__item list__item--last">2</li></ul>' +
  '<div class="Card_card__x Card_wide__y" data-kind="news"><h2 class="Card_title__z">t</h2><p></p><span class="md:flex">s</span></div>' +
  '<input type="checkbox" checked class="toggle"><label for="t" class="toggle__label">l</label></main>' +
  '<footer class="footer"><p class="footer__text">f</p><b class="tab\tsep\nnl">w</b><i id=" sp "></i></footer>';

const CSS = `
*{box-sizing:border-box}
:root{--c:red}
html,body{margin:0}
html.no-js .nav{display:none}
html[lang=ru] .nav{color:red}
:before,:after{content:""}
.Header_root__a1 .nav__link{color:blue}
.Header_root__a1 > .nav{display:flex}
.Missing_root .nav__link{color:green}
.nav__link.is-active:hover{color:red}
.nav__link:not(.is-active){opacity:.5}
.nav__link:is(.is-active, .nope){font-weight:700}
.list__item + .list__item{margin:0}
.list__item ~ .list__item--last{margin:1px}
.list__item:first-child{margin:2px}
.list__item:nth-child(2n){margin:3px}
.list__item:last-child::after{content:"!"}
.list > :last-child{margin:4px}
ul li{padding:0}
ol li{padding:1px}
[data-kind=news] .Card_title__z{font-size:2em}
[data-kind=sport] .Card_title__z{font-size:3em}
.Card_card__x:has(.Card_title__z){gap:1px}
.Card_card__x:has(.Nope){gap:2px}
.Card_card__x p:empty{display:none}
.md\\:flex{display:flex}
#main-nav a{color:#000}
#other a{color:#111}
.toggle:checked + .toggle__label{color:red}
.footer .footer__text, .absent .x{font-size:12px}
.absent, .footer{border:0}
div.Card_wide__y.Card_card__x{width:100%}
DIV.Card_wide__y{height:1px}
[class~=footer]{a:b}
[class~=FOOTER i]{a:c}
body > *{a:d}
main *{a:e}
.a .b .c .d .e{a:f}
html[lang=ru]{a:g}
html.no-js{a:h}
footer .sep, footer .nl{a:i}
.sep{a:j}
.tab{a:k}
#sp{a:l}
.page-root{a:m}
.inline{a:n}
[class]{a:o}
@media (max-width:600px){.nav__link{font-size:12px}.absent{x:y}}
@supports (display:grid){.list{display:grid}.absent2{x:y}}
@keyframes spin{from{opacity:0}to{opacity:1}}
@keyframes unused{from{opacity:0}to{opacity:1}}
.nav{animation:spin 1s}
@font-face{font-family:F;src:url(/f.woff2)}
.unknown:hoverx{a:b}
.list__item:nth-child(foo){a:b}
.footer:unknownfn(1){a:b}
`;

describe('createFastCritters: output identical to critters', () => {
  it('a page with combinators, pseudo-classes, at-rules and failing selectors', async () => {
    const { original, fast } = await both(page(BODY, [writeSheet(CSS)]));
    expect(fast).toBe(original);
    // Проверка фикстуры: в ней есть и оставленные, и выброшенные правила.
    expect(original).toContain('.Header_root__a1 .nav__link');
    expect(original).not.toContain('Missing_root');
  });

  it('several stylesheets, one with nothing critical (its style tag is removed mid-run)', async () => {
    // critters вырезает псевдоклассы без скобок, поэтому позиции записаны как :nth-child(1).
    // Вопрос задаётся, пока первый <style> стоит первым в <head> (false), и снова после его удаления (true);
    // сам отсоединённый <style> совпадением не считается.
    const empty = writeSheet('.nothing-here{a:b}head > link:nth-child(1){c:d}');
    const sibling = writeSheet(
      'head > link:nth-child(1){e:f}style:nth-child(1){g:h}head > style + style{i:j}style:nth-of-type(2){k:l}'
    );
    const { original, fast } = await both(
      page(BODY, [empty, writeSheet(CSS), sibling])
    );
    expect(fast).toBe(original);
    expect(original).not.toContain('c:d');
    expect(original).toContain('head > link:nth-child(1){e:f}');
    expect(original).not.toContain('style:nth-child(1)');
  });

  it('existing inline styles and noscript links', async () => {
    const html = page(
      BODY,
      [writeSheet(CSS)],
      '<style class="inline">.x{a:b}</style><noscript><link class="ns"></noscript>'
    );
    const { original, fast } = await both(html);
    expect(fast).toBe(original);
  });

  it('a custom data-critters-container (falls back to the original matcher)', async () => {
    const html = page(
      `<div data-critters-container class="wrap">${BODY}</div><aside class="outside"></aside>`,
      [writeSheet(`${CSS}.outside{a:b}.wrap .nav{c:d}`)]
    );
    const { original, fast } = await both(html);
    expect(fast).toBe(original);
  });

  it('missing and remote stylesheets', async () => {
    const html = page(BODY, [
      '/_next/missing.css',
      'https://cdn.example.com/a.css',
      writeSheet(CSS),
    ]);
    const { original, fast } = await both(html);
    expect(fast).toBe(original);
  });

  it('sees the class/id set as it was before critters edited the DOM', async () => {
    // Маленький стиль встраивается целиком, а его <link class="css-link"> удаляется до сопоставления.
    const small = writeSheet('.tiny{a:b}');
    const big = writeSheet(`${CSS}.css-link{c:d}#css-link-id{e:f}`);
    const html = page(BODY, [big]).replace(
      '</head>',
      `<link rel="stylesheet" href="${small}" class="css-link" id="css-link-id"/></head>`
    );
    const { original, fast } = await both(html, { inlineThreshold: 20 });
    expect(fast).toBe(original);
    expect(original).toContain('.css-link{c:d}');
  });

  it('pruneSource and inlineThreshold options', async () => {
    const html = page(BODY, [writeSheet(CSS), writeSheet('.footer{a:b}')]);
    for (const options of [
      { pruneSource: true },
      { inlineThreshold: 20 },
      { keyframes: 'all' },
    ]) {
      const { original, fast } = await both(html, options);
      expect(fast).toBe(original);
    }
  });
});

describe('createFastCritters: concurrent documents', () => {
  it('gives each document the result it gets alone (stock critters does not)', async () => {
    const sheet = writeSheet(CSS);
    const pages = [
      page(BODY, [sheet]),
      page('<div class="only-here"><p class="sep">x</p></div>', [
        writeSheet('.only-here{a:b}.sep{c:d}.footer{e:f}'),
      ]),
      page('<section class="footer"></section>', [sheet]),
    ];
    const alone = [];
    for (const html of pages) {
      alone.push(await new Critters(nextOptions(dir)).process(html));
    }

    const together = await Promise.all(
      pages.map((html) => new FastCritters(nextOptions(dir)).process(html))
    );
    expect(together).toEqual(alone);

    const stock = await Promise.all(
      pages.map((html) => new Critters(nextOptions(dir)).process(html))
    );
    expect(stock).not.toEqual(alone);
  });
});

describe('createFastCritters: randomized equivalence', () => {
  // Детерминированный ГПСЧ: падение воспроизводится по сиду.
  const rng = (seed: number) => () => {
    seed = (seed * 1103515245 + 12345) & 0x7fffffff;
    return seed / 0x7fffffff;
  };

  const TAGS = ['div', 'span', 'p', 'ul', 'li', 'a', 'section', 'b'];
  const CLASSES = ['a', 'b', 'c', 'd', 'X', 'e-f', 'g_h'];
  const IDS = ['i1', 'i2', 'i3'];
  const COMBINATORS = [' ', ' > ', ' + ', ' ~ '];
  const PSEUDOS = [
    '',
    '',
    '',
    ':first-child',
    ':last-child',
    ':nth-child(2)',
    ':not(.a)',
    ':empty',
    ':hover',
    '::before',
  ];

  const randomHtml = (rand: () => number) => {
    const pick = <T>(xs: T[]) => xs[Math.floor(rand() * xs.length)];
    const node = (depth: number): string => {
      const tag = pick(TAGS);
      const classes = CLASSES.filter(() => rand() < 0.2).join(
        pick([' ', '  ', '\t', '\n '])
      );
      const id = rand() < 0.1 ? ` id="${pick(IDS)}"` : '';
      const kids =
        depth < 4
          ? Array.from({ length: Math.floor(rand() * 4) }, () =>
              node(depth + 1)
            ).join('')
          : '';
      return `<${tag}${classes ? ` class="${classes}"` : ''}${id}>${kids}</${tag}>`;
    };
    return Array.from({ length: 3 }, () => node(0)).join('');
  };

  const randomSelector = (rand: () => number) => {
    const pick = <T>(xs: T[]) => xs[Math.floor(rand() * xs.length)];
    const compound = () => {
      const parts = [
        rand() < 0.4 ? pick(TAGS) : '',
        rand() < 0.7 ? `.${pick(CLASSES)}` : '',
        rand() < 0.1 ? `#${pick(IDS)}` : '',
      ];
      const head = parts.join('') || '*';
      return head + pick(PSEUDOS);
    };
    let selector = compound();
    for (let i = Math.floor(rand() * 3); i > 0; i--) {
      selector =
        selector.replace(/::before$/, '') + pick(COMBINATORS) + compound();
    }
    return selector;
  };

  it('matches critters on 300 random pages', async () => {
    for (let seed = 1; seed <= 300; seed++) {
      const rand = rng(seed);
      const rules = Array.from(
        { length: 40 },
        (_, i) => `${randomSelector(rand)}{--r:${i}}`
      ).join('\n');
      const { original, fast } = await both(
        page(randomHtml(rand), [writeSheet(rules)])
      );
      if (fast !== original) {
        throw new Error(`seed ${seed} differs`);
      }
    }
  });
});

describe('createFastCritters: stylesheet cache', () => {
  it('reads a stylesheet from disk once across documents', async () => {
    const href = writeSheet('.footer{a:b}');
    const spy = vi.spyOn(fs, 'readFile');
    try {
      const Fast = createFastCritters(Critters, deps);
      await new Fast(nextOptions(dir)).process(page(BODY, [href]));
      await new Fast(nextOptions(dir)).process(page(BODY, [href]));
      expect(
        spy.mock.calls.filter(([file]) =>
          String(file).endsWith(href.replace('/_next/', ''))
        )
      ).toHaveLength(1);
    } finally {
      spy.mockRestore();
    }
  });

  it('does not cache a failed read', async () => {
    const Fast = createFastCritters(Critters, deps);
    const html = page(BODY, ['/_next/late.css']);
    const before = await new Fast(nextOptions(dir)).process(html);
    fs.writeFileSync(join(dir, 'late.css'), '.footer{late:1}');
    const after = await new Fast(nextOptions(dir)).process(html);
    expect(before).not.toContain('late:1');
    expect(after).toContain('late:1');
  });

  it('can be turned off', async () => {
    const href = writeSheet('.footer{a:b}');
    const spy = vi.spyOn(fs, 'readFile');
    try {
      const Fast = createFastCritters(Critters, {
        ...deps,
        isFileCacheEnabled: false,
      });
      await new Fast(nextOptions(dir)).process(page(BODY, [href]));
      await new Fast(nextOptions(dir)).process(page(BODY, [href]));
      expect(
        spy.mock.calls.filter(([file]) =>
          String(file).endsWith(href.replace('/_next/', ''))
        )
      ).toHaveLength(2);
    } finally {
      spy.mockRestore();
    }
  });
});
