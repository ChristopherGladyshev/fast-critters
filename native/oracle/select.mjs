// Оракул для crates/select: сравнивает Rust-порт с собственными копиями
// css-what / css-select внутри critters (selectOne, selectAll, parse, ошибки компиляции).
//
//   node oracle/select.mjs real              15 реальных страниц x все реальные селекторы (исходные + преобразованные critters)
//   node oracle/select.mjs fuzz [seed] [n]   n случайных DOM x случайные селекторы (по умолчанию 400)
//   node oracle/select.mjs time [page]       замер select_one, все реальные селекторы, контекст <html>
//   node oracle/select.mjs golden            выводит golden-кейсы юнит-тестов (исходник Rust)
//
// Сначала сборка: CARGO_TARGET_DIR=target-select cargo build --release -p select --example oracle
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '..');
const pkg = path.resolve(root, '..');
const req = createRequire(
  path.join(pkg, 'node_modules/critters-upstream/package.json')
);
const cssSelect = req('css-select');
const cssWhat = req('css-what');
const htmlparser2 = req('htmlparser2');
const postcss = req('postcss');
const BIN =
  process.env.SELECT_ORACLE_BIN ||
  path.join(root, 'target-select/release/examples/oracle');
// Сохранённые страницы p<N>.html для режима `real`.
const PAGES = process.env.SELECT_PAGES;
// CSS собранного приложения (`.next/static/css`) для режима `real`.
const CSS_DIR = process.env.ORACLE_CSS_DIR;

function crittersParse() {
  const src = fs.readFileSync(
    path.join(pkg, 'node_modules/critters-upstream/dist/critters.js'),
    'utf8'
  );
  const start = src.indexOf('var SelectorType;');
  const end = src.indexOf('function parseSelector(');
  const endFn = src.indexOf('\n/**', end);
  // eslint-disable-next-line no-new-func
  return new Function(src.slice(start, endFn) + '\nreturn parse;')();
}

function parseHtml(html) {
  return htmlparser2.parseDocument(html, { decodeEntities: false });
}
function serialize(node) {
  switch (node.type) {
    case 'root':
      return ['doc', node.children.map(serialize)];
    case 'tag':
    case 'script':
    case 'style':
      return [
        'e',
        node.name,
        Object.keys(node.attribs).map((k) => [k, node.attribs[k]]),
        node.children.map(serialize),
      ];
    case 'text':
      return ['t', node.data];
    case 'comment':
      return ['c', node.data];
    case 'directive':
      return ['d', node.name, node.data];
    case 'cdata':
      return ['x', node.children.map(serialize)];
    default:
      throw new Error('node type ' + node.type);
  }
}
function indexNodes(doc) {
  const list = [];
  const walk = (n) => {
    list.push(n);
    if (n.children) n.children.forEach(walk);
  };
  walk(doc);
  const map = new Map(list.map((n, i) => [n, i]));
  return { list, map };
}

function jsParse(sel) {
  try {
    return JSON.stringify(cssWhat.parse(sel));
  } catch (e) {
    return JSON.stringify({ err: e.message });
  }
}
function jsCase(sel, ctx, map, onlyOne) {
  let c = null;
  try {
    cssSelect._compileUnsafe(sel, undefined, ctx);
  } catch (e) {
    c = e.message;
  }
  let one,
    all = null;
  try {
    const r = cssSelect.selectOne(sel, ctx);
    one = r ? map.get(r) : null;
  } catch (e) {
    one = { err: e.message };
  }
  if (!onlyOne) {
    try {
      all = cssSelect.selectAll(sel, ctx).map((n) => map.get(n));
    } catch (e) {
      all = { err: e.message };
    }
  }
  return { c, one, all };
}

function runRust(lines, extraArgs = []) {
  const res = spawnSync(BIN, extraArgs, {
    input: lines.join('\n') + '\n',
    maxBuffer: 1 << 30,
  });
  if (res.status !== 0)
    throw new Error('rust oracle failed: ' + res.stderr.toString());
  return {
    out: res.stdout
      .toString()
      .trim()
      .split('\n')
      .map((l) => JSON.parse(l)),
    stderr: res.stderr.toString(),
  };
}

const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

// cases: [{ doc, contexts: [node], selectors: [..], onlyOne }]
function check(cases, label) {
  const lines = [];
  const expected = [];
  for (const k of cases) {
    const { map } = indexNodes(k.doc);
    const ctxIdx = k.contexts.map((n) => map.get(n));
    lines.push(
      JSON.stringify({
        dom: serialize(k.doc),
        contexts: ctxIdx,
        selectors: k.selectors,
        onlyOne: !!k.onlyOne,
      })
    );
    expected.push(
      k.selectors.map((sel) => ({
        p: jsParse(sel),
        x: k.contexts.map((ctx) => jsCase(sel, ctx, map, k.onlyOne)),
      }))
    );
  }
  const { out } = runRust(lines);
  let evals = 0,
    mismatches = 0,
    throws = 0,
    matched = 0,
    runtimeThrows = 0;
  const shown = [];
  out.forEach((r, i) => {
    expected[i].forEach((exp, j) => {
      const got = r[j];
      const sel = cases[i].selectors[j];
      if (JSON.stringify(got.p) !== exp.p) {
        mismatches++;
        if (shown.length < 30)
          shown.push({
            sel,
            kind: 'parse',
            js: exp.p,
            rust: JSON.stringify(got.p),
          });
      }
      exp.x.forEach((e, k) => {
        evals++;
        if (e.c) throws++;
        if (typeof e.one === 'number') matched++;
        if (!e.c && (e.one?.err || e.all?.err)) runtimeThrows++;
        const g = got.x[k];
        if (!eq(e.c, g.c) || !eq(e.one, g.one) || !eq(e.all, g.all)) {
          mismatches++;
          if (shown.length < 30) shown.push({ sel, ctx: k, js: e, rust: g });
        }
      });
    });
  });
  for (const s of shown)
    console.log('MISMATCH', JSON.stringify(s).slice(0, 1500));
  console.log(
    `${label}: ${cases.length} doms, ${evals} selector evaluations (${throws} compile throws, ${runtimeThrows} match-time throws, ${matched} selectOne hits), ${mismatches} mismatches`
  );
  return mismatches;
}

function realSelectors() {
  if (!CSS_DIR) throw new Error('ORACLE_CSS_DIR is not set');
  const set = new Set();
  for (const f of fs
    .readdirSync(CSS_DIR)
    .filter((f) => f.endsWith('.css'))
    .sort()) {
    const ast = postcss.parse(fs.readFileSync(path.join(CSS_DIR, f), 'utf8'));
    ast.walkRules((rule) => {
      if (
        rule.parent &&
        rule.parent.type === 'atrule' &&
        /keyframes/i.test(rule.parent.name)
      )
        return;
      for (const sel of rule.selectors) {
        set.add(sel);
        const t = sel
          .replace(/(?<!\\)::?[a-z-]+(?![a-z-(])/gi, '')
          .replace(/::?not\(\s*\)/g, '')
          .replace(/\(\s*,/g, '(')
          .replace(/,\s*\)/g, ')')
          .trim();
        if (t) set.add(t);
      }
    });
  }
  return [...set];
}
function pagesDir() {
  if (!PAGES) {
    throw new Error('set SELECT_PAGES to a directory of saved pages');
  }
  return PAGES;
}
function pages() {
  const dir = pagesDir();
  return fs
    .readdirSync(dir)
    .filter((f) => /^p\d+\.html$/.test(f))
    .sort()
    .map((f) => path.join(dir, f));
}
function htmlElement(doc) {
  return doc.children.find((c) => String(c.name).toLowerCase() === 'html');
}

function real() {
  const selectors = realSelectors();
  const cp = crittersParse();
  let parseDiff = 0;
  for (const s of selectors) {
    let a, b;
    try {
      a = JSON.stringify(cp(s));
    } catch (e) {
      a = 'E' + e.message;
    }
    try {
      b = JSON.stringify(cssWhat.parse(s));
    } catch (e) {
      b = 'E' + e.message;
    }
    if (a !== b) parseDiff++;
  }
  console.log(
    `critters-bundled parse vs css-what parse: ${parseDiff} differences over ${selectors.length} selectors`
  );
  let total = 0;
  for (const p of pages()) {
    const doc = parseHtml(fs.readFileSync(p, 'utf8'));
    total += check(
      [{ doc, contexts: [htmlElement(doc), doc], selectors }],
      path.basename(p)
    );
  }
  console.log(`REAL TOTAL mismatches: ${total}`);
  return total;
}

function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function fuzzGen(seed) {
  const R = rng(seed);
  const int = (n) => Math.floor(R() * n);
  const pick = (a) => a[int(a.length)];
  const chance = (p) => R() < p;

  const TAGS = [
    'div',
    'span',
    'p',
    'a',
    'ul',
    'li',
    'input',
    'option',
    'select',
    'optgroup',
    'fieldset',
    'legend',
    'button',
    'h1',
    'h2',
    'template',
    'br',
    'section',
    'b',
    'i',
    'img',
    'textarea',
    'link',
    'area',
    'Div',
    'SPAN',
  ];
  const CLASSES = [
    'a',
    'b',
    'c',
    'foo',
    'Foo',
    'bar',
    'x-y',
    'x',
    'é',
    'ÄB',
    'ä',
    'q1',
    'is-active',
    'hover',
    'ſ',
    'K',
  ];
  const WORDS = [
    'foo',
    'Bar',
    'baz qux',
    '',
    'x',
    'en-US',
    'en',
    'EN',
    'text',
    'checkbox',
    'radio',
    'Radio',
    'submit',
    'button',
    'a b',
    ' a',
    'b ',
    'é-x',
    'İ',
    'ß',
    'σς',
    '{',
    '-',
    'stylesheet',
    'Stylesheet',
  ];
  const ATTRS = [
    'id',
    'href',
    'type',
    'lang',
    'rel',
    'data-x',
    'title',
    'name',
    'value',
    'dir',
    'disabled',
    'selected',
    'multiple',
    'checked',
    'required',
    'constructor',
    'hreflang',
    'media',
    'target',
    'data-critters-container',
  ];

  function genNode(depth) {
    const r = R();
    if (depth > 4 || r < 0.25) {
      const t = R();
      if (t < 0.6)
        return pick([
          'hello',
          'Foo bar',
          ' ',
          'x',
          'ÄÖ text',
          'null',
          'a\nb',
          '',
        ]);
      if (t < 0.8) return '<!--c-->';
      return '<br>';
    }
    const tag = pick(TAGS);
    let attrs = '';
    const na = int(4);
    for (let i = 0; i < na; i++) {
      if (chance(0.45)) {
        const cls = Array.from({ length: 1 + int(3) }, () =>
          pick(CLASSES)
        ).join(pick([' ', '  ', '\t', ' ', '\u00a0', '\u2003', '\n']));
        attrs += ` class="${chance(0.1) ? '' : cls}"`;
      } else {
        const a = pick(ATTRS);
        attrs += chance(0.2) ? ` ${a}` : ` ${a}="${pick(WORDS)}"`;
      }
    }
    if (chance(0.3)) attrs += ` id="${pick(['a', 'b', 'main', 'A', 'x y'])}"`;
    const voids = ['br', 'input', 'img', 'link', 'area'];
    if (voids.includes(tag.toLowerCase())) return `<${tag}${attrs}>`;
    const n = int(5);
    let inner = '';
    for (let i = 0; i < n; i++) inner += genNode(depth + 1);
    return `<${tag}${attrs}>${inner}</${tag}>`;
  }
  function genDoc() {
    let body = '';
    const n = 1 + int(6);
    for (let i = 0; i < n; i++) body += genNode(0);
    const pre = chance(0.5) ? '<!doctype html>' : '';
    if (chance(0.15)) return pre + body; // без обёртки <html>
    return `${pre}<html lang="en"><head><title>t</title></head><body class="${pick(CLASSES)}">${body}</body></html>${chance(0.2) ? '<p>after</p>' : ''}`;
  }

  const ident = () =>
    pick([
      'a',
      'b',
      'c',
      'foo',
      'Foo',
      'FOO',
      'bar',
      'x-y',
      'é',
      'ä',
      'ÄB',
      'q1',
      'is-active',
      'x',
      'K',
      'ſ',
      '\\66 oo',
      'f\\oo',
      '\\31 x',
      '-x',
      '_a',
      'hover',
    ]);
  const tagSel = () =>
    pick([
      'div',
      'span',
      'p',
      'a',
      'li',
      'ul',
      'input',
      'option',
      'select',
      'body',
      'html',
      'head',
      'DIV',
      'Span',
      'h1',
      'button',
      'legend',
      'fieldset',
      'template',
      'b',
      'img',
      '*',
      'br',
    ]);
  const attrName = () =>
    pick([
      'id',
      'href',
      'type',
      'lang',
      'rel',
      'data-x',
      'title',
      'class',
      'CLASS',
      'name',
      'dir',
      'disabled',
      'constructor',
      'hreflang',
      'Type',
      'value',
      'missing',
      'data-critters-container',
    ]);
  const attrVal = () =>
    pick([
      'foo',
      'Foo',
      '"foo"',
      "'Bar'",
      '"baz qux"',
      '""',
      'x',
      'en',
      'EN',
      '"en-US"',
      'text',
      'checkbox',
      'radio',
      '"a b"',
      'é',
      '"İ"',
      '"ß"',
      '"σς"',
      '{',
      '-',
      '"\\66 oo"',
      'stylesheet',
      'a',
      'b',
      'é-x',
      '"x"',
    ]);
  const nth = () =>
    pick([
      '1',
      '2',
      'odd',
      'even',
      'ODD',
      '2n+1',
      '2n + 1',
      ' -n+3 ',
      'n',
      '-n',
      '0',
      '-2n+5',
      '3n-1',
      '+3',
      '-1',
      'n+0',
      '2n+',
      'x',
      '',
      '0n+0',
      '-0n+2',
      '10n',
      '1 n',
      '+ 2n',
    ]);
  function simple(depth) {
    const r = R();
    if (r < 0.18) return tagSel();
    if (r < 0.36) return '.' + ident();
    if (r < 0.44) return '#' + pick(['a', 'b', 'main', 'A', 'x', 'missing']);
    if (r < 0.62) {
      const op = pick(['', '=', '~=', '|=', '^=', '$=', '*=', '!=']);
      if (!op) return `[${attrName()}]`;
      const flag = pick(['', '', '', ' i', ' s', ' I', 'i']);
      return `[${pick(['', ' '])}${attrName()}${op}${attrVal()}${flag}]`;
    }
    return pseudo(depth);
  }
  function pseudo(depth) {
    const plain = [
      'first-child',
      'last-child',
      'only-child',
      'first-of-type',
      'last-of-type',
      'only-of-type',
      'empty',
      'root',
      'scope',
      'hover',
      'visited',
      'active',
      'link',
      'any-link',
      'disabled',
      'enabled',
      'checked',
      'required',
      'optional',
      'selected',
      'checkbox',
      'radio',
      'text',
      'parent',
      'header',
      'button',
      'input',
      'submit',
      'unknown',
      'focus',
      'constructor',
      'First-Child',
      'file',
      'password',
      'reset',
      'image',
    ];
    const r = R();
    if (r < 0.35) return ':' + pick(plain);
    if (r < 0.55)
      return `:${pick(['nth-child', 'nth-last-child', 'nth-of-type', 'nth-last-of-type', 'NTH-CHILD'])}(${nth()})`;
    if (r < 0.62)
      return `:${pick(['contains', 'icontains'])}(${pick(['foo', '"Foo"', "'bar'", 'hello', 'HELLO', '', 'null', 'x', '"a\\"b"'])})`;
    if (r < 0.66)
      return pick([
        ':first-child(2)',
        ':empty()',
        ':link(a)',
        ':nth-child',
        ':icontains',
        ':contains',
        ':foo(bar)',
        '::before',
        ':root(x)',
        ':scope()',
        ':host(a)',
        ':host-context(a, b c)',
        ':is("a")',
      ]);
    if (depth > 2) return ':' + pick(plain);
    const name = pick([
      'is',
      'not',
      'has',
      'where',
      'matches',
      'not',
      'has',
      'is',
    ]);
    const n = 1 + int(2);
    const parts = [];
    for (let i = 0; i < n; i++)
      parts.push(complex(depth + 1, name === 'has' || chance(0.2)));
    return `:${name}(${parts.join(pick([', ', ',']))})`;
  }
  function compound(depth) {
    let s = '';
    const n = chance(0.5) ? 1 : 1 + int(3);
    for (let i = 0; i < n; i++) {
      const p = simple(depth);
      if (i > 0 && /^[a-zA-Z*]/.test(p)) continue; // тег должен идти первым
      s += p;
    }
    return s || '*';
  }
  function complex(depth, relative) {
    let s =
      relative && chance(0.6) ? pick(['> ', '+ ', '~ ', '>', '+', '~']) : '';
    const n = chance(0.35) ? 1 : 1 + int(depth > 0 ? 2 : 3);
    for (let i = 0; i < n; i++) {
      if (i > 0) s += pick([' ', ' > ', '>', ' + ', ' ~ ', '  ', ' ', ' ']);
      s += compound(depth);
    }
    return s;
  }
  function selector() {
    const r = R();
    if (r < 0.04)
      return pick([
        '',
        ' ',
        'a,',
        ',a',
        'a,,b',
        'a >',
        'a > > b',
        '[',
        '[a',
        '[a=',
        '[a="b]',
        '[a=b x]',
        '[a^b]',
        ':',
        '::',
        '#',
        '.',
        'a)',
        ':is(a',
        ':not()',
        ':has()',
        '/* c */ a',
        'a /* c */ b',
        'a/*',
        '|a',
        '*|a',
        '[a|b]',
        '[*|b]',
        'a || b',
        '*|*',
        'ns|*',
        'a\\',
        '.a\\',
        '<a',
        'a < b',
        ':is(a)b',
        ':nth-child(2n+1 of a)',
        '[a=b]]',
        'a\t>\nb',
        '. a',
        '#\\31',
        ':IS(a)',
        ':not(a b)',
        'a:not(:has(b))',
        ':is(> a)',
        ':has(> a b)',
        ':has(+ a)',
        ':has(~ b)',
        ':scope > body',
        '> body',
        '+ p',
        '~ p',
        ':scope div',
        ':is(:scope a)',
        'div:has(:scope)',
      ]);
    const n = chance(0.15) ? 2 : 1;
    const parts = [];
    for (let i = 0; i < n; i++) parts.push(complex(0, chance(0.08)));
    return parts.join(pick([', ', ',']));
  }

  return { genDoc, selector, int, pick, chance };
}

function fuzz(seed = 1, n = 400) {
  const cases = [];
  let evals = 0;
  for (let d = 0; d < n; d++) {
    const g = fuzzGen(seed * 100003 + d);
    const doc = parseHtml(g.genDoc());
    const { list } = indexNodes(doc);
    const elements = list.filter(
      (x) => x.type === 'tag' || x.type === 'script' || x.type === 'style'
    );
    const contexts = [doc];
    const html = htmlElement(doc);
    if (html) contexts.push(html);
    // более глубокий контейнер (родитель — элемент)
    const deep = elements.filter(
      (e) => e.parent && e.parent.type === 'tag' && e.parent.name !== undefined
    );
    if (deep.length) {
      const c = deep[g.int(deep.length)];
      c.attribs['data-critters-container'] = '';
      contexts.push(c);
    }
    const selectors = Array.from({ length: 25 }, () => g.selector());
    evals += selectors.length * contexts.length;
    cases.push({ doc, contexts, selectors });
  }
  return check(cases, `fuzz seed=${seed}`);
}

function time(pageName = 'p13.html') {
  const selectors = realSelectors();
  const doc = parseHtml(
    fs.readFileSync(path.join(pagesDir(), pageName), 'utf8')
  );
  const html = htmlElement(doc);
  // JS: selectOne на каждый селектор (с компиляцией, т. к. critters вызывает его со строкой)
  for (let w = 0; w < 2; w++)
    for (const s of selectors) {
      try {
        cssSelect.selectOne(s, html);
      } catch {}
    }
  const runs = 5;
  let best = Infinity;
  for (let r = 0; r < runs; r++) {
    const t0 = process.hrtime.bigint();
    for (const s of selectors) {
      try {
        cssSelect.selectOne(s, html);
      } catch {}
    }
    best = Math.min(best, Number(process.hrtime.bigint() - t0) / 1e6);
  }
  const { map } = indexNodes(doc);
  const line = JSON.stringify({
    dom: serialize(doc),
    contexts: [map.get(html)],
    selectors,
    onlyOne: true,
  });
  if (process.env.SELECT_DUMP)
    fs.writeFileSync(process.env.SELECT_DUMP, line + '\n');
  const rust = [];
  for (let r = 0; r < runs; r++)
    rust.push(runRust([line], ['--time']).stderr.trim());
  console.log(`${pageName}: ${selectors.length} selectors, ${map.size} nodes`);
  console.log(
    `JS css-select selectOne (string -> parse+compile+search), best of ${runs}: ${best.toFixed(2)} ms`
  );
  console.log('Rust (release) runs:\n  ' + rust.join('\n  '));
}

// golden-кейсы для crates/select/tests/golden.rs
function golden() {
  const html =
    '<!doctype html><html lang="en"><head><title>T</title></head><body class="Main x"><div id="a" class="foo bar" data-x="en-US"><p class="foo">Hello <b>World</b></p><p></p><span lang="EN" title="a b">x</span><a href="/x" rel="Stylesheet">l</a></div><ul><li>1</li><li class="c">2</li><li>3</li><!--c--><li>4</li></ul><input type="Checkbox" checked><select><option>o1</option><option selected>o2</option></select><section data-critters-container><div class="in"><p>deep</p></div></section></body></html>';
  const doc = parseHtml(html);
  const { map } = indexNodes(doc);
  const htmlEl = htmlElement(doc);
  const container = cssSelect.selectOne('section', doc);
  const sels = [
    'div',
    'DIV',
    '.foo',
    '.Foo',
    '#a',
    'p.foo b',
    'div > p',
    'p + span',
    'p ~ a',
    'li:nth-child(2)',
    'li:nth-child(odd)',
    'li:nth-last-child(1)',
    'li:nth-of-type(2n)',
    'li:first-child',
    'li:last-child',
    'p:empty',
    'p:only-of-type',
    ':contains(World)',
    ':icontains(world)',
    'p:contains("Hello")',
    '[lang|=en]',
    '[lang|=EN s]',
    '[data-x|=en]',
    '[rel=stylesheet]',
    '[rel=stylesheet s]',
    '[title~=b]',
    '[title~="a b"]',
    '[href^="/"]',
    '[href$=x]',
    '[href*=""]',
    '[data-x!=en-US]',
    ':not(p)',
    'div:has(> b)',
    'div:has(b)',
    'li:has(+ li.c)',
    ':is(span, a)[lang]',
    'input:checked',
    'option:selected',
    ':checkbox',
    ':root',
    ':scope',
    ':scope > body',
    '> body',
    '+ x',
    'body > :first-child',
    'a:link',
    'a:any-link',
    ':header',
    'li:nth-child(-n+2)',
    'li:nth-child(2n+)',
    ':foo',
    '::before',
    'a::after',
    '[x',
    ':is(a',
    'a,',
    '|a',
    '[a|b]',
    ':hover',
    'p:first-child(1)',
    ':nth-child',
    'div p',
    'section p',
    '.in p',
    'html body',
    'html > body',
    ':root p',
    ':is(p p)',
    'li:not(li li)',
    'b:not(div b)',
    'div:has(div p)',
    'p:not(.foo)',
    '[lang=en]',
    '[LANG=En]',
    '[title=A I]',
  ];
  const ctxs = [
    ['doc', doc],
    ['html', htmlEl],
    ['container', container],
  ];
  const out = [];
  for (const sel of sels) {
    for (const [cn, ctx] of ctxs) {
      const e = jsCase(sel, ctx, map, false);
      out.push(
        `    (${JSON.stringify(sel)}, ${JSON.stringify(cn)}, ${e.c === null ? 'None' : 'Some(' + JSON.stringify(e.c) + ')'}, ${typeof e.one === 'number' ? 'Some(' + e.one + ')' : 'None'}, &[${(Array.isArray(e.all) ? e.all : []).join(', ')}]),`
      );
    }
  }
  console.log(
    `// html: ${JSON.stringify(html)}\n// context ids: html=${map.get(htmlEl)} container=${map.get(container)}`
  );
  console.log(JSON.stringify(serialize(doc)));
  console.log(out.join('\n'));
}

const [mode = 'real', a1, a2] = process.argv.slice(2);
if (mode === 'real') process.exitCode = real() ? 1 : 0;
else if (mode === 'fuzz')
  process.exitCode = fuzz(Number(a1 || 1), Number(a2 || 400)) ? 1 : 0;
else if (mode === 'time') time(a1);
else if (mode === 'golden') golden();
