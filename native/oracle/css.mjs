// Оракул для crates/css: настоящий `processStyle` из critters 0.0.23 (postcss 8.5.8)
// против Rust-порта (crates/css/examples/oracle.rs).
//
// Запуск из native/ (сначала сборка:
//   CARGO_TARGET_DIR=$PWD/target-css cargo build --release --example oracle -p css):
//   node oracle/css.mjs                       # fuzz 6000 + реальные файлы
//   node oracle/css.mjs fuzz [n] [seed0]      # генератор с seed
//   node oracle/css.mjs real                  # реальный CSS Next x seeds (-1, -2, 1..4)
//   node oracle/css.mjs case '<css>' [seed]   # вывести JS + Rust для одного входа
//   node oracle/css.mjs golden [n]            # вывести golden-тесты Rust в stdout
//   node oracle/css.mjs bench                 # замер времени JS на реальных файлах
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, readdirSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

const require = createRequire(
  new URL('../../node_modules/critters-upstream/package.json', import.meta.url)
);
const Critters = require('critters-upstream');

const ROOT = new URL('..', import.meta.url).pathname;
const TARGET = process.env.CARGO_TARGET_DIR || join(ROOT, 'target-css');
const BIN = join(TARGET, 'release/examples/oracle');
// CSS собранного приложения (`.next/static/css`) для режима `real`.
const REAL_DIR = process.env.ORACLE_CSS_DIR;
const OUT = join(ROOT, 'oracle/out');

function fnv(s) {
  let h = 0x811c9dc5;
  for (const b of Buffer.from(s, 'utf8')) {
    h ^= b;
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h >>> 0;
}
function pred(seed, sel) {
  if (seed === -1) return true;
  if (seed === -2) return false;
  const m = ((fnv(sel) ^ seed) >>> 0) % 7;
  if (m === 0) throw new Error('boom:' + sel);
  return m > 3;
}

const critters = new Critters({
  ssrMode: true,
  reduceInlineStyles: false,
  path: '/x',
  publicPath: '/_next/',
  preload: 'media',
  fonts: false,
  logLevel: 'silent',
});
let warnings = [];
critters.logger = {
  warn(m) {
    warnings.push(m);
  },
  info() {},
  error() {},
  debug() {},
  trace() {},
};

async function runJs(css, seed) {
  const calls = [];
  warnings = [];
  const style = {
    $$name: 'x.css',
    _text: css,
    removed: false,
    get textContent() {
      return this._text;
    },
    set textContent(v) {
      this._text = v;
    },
    parentNode: {},
    remove() {
      this.removed = true;
    },
  };
  const document = {
    crittersContainer: {
      exists(sel) {
        calls.push(sel);
        return pred(seed, sel);
      },
    },
  };
  let threw = false;
  let message;
  try {
    await critters.processStyle(style, document);
  } catch (e) {
    threw = true;
    message = String(e && e.message);
  }
  const warn = warnings.length ? warnings.join('\u0000') : null;
  if (threw) return { threw, message, calls, warn };
  let out;
  if (css === '') out = { skipped: true };
  else if (style.removed) out = { removed: true };
  else out = { css: style._text };
  return { threw, out, warn, calls };
}

function normRust(r) {
  if (r.threw) return r;
  let out;
  if (r.skipped) out = { skipped: true };
  else if (r.css.trim().length === 0) out = { removed: true };
  else out = { css: r.css };
  const warn = r.failed.length
    ? `${r.failed.length} rules skipped due to selector errors:\n  ${r.failed.join('\n  ')}`
    : null;
  return { threw: false, out, warn, calls: r.calls };
}

function runRust(cases) {
  mkdirSync(OUT, { recursive: true });
  const inPath = join(OUT, 'css-in.jsonl');
  writeFileSync(inPath, cases.map((c) => JSON.stringify(c)).join('\n') + '\n');
  const r = spawnSync(BIN, [inPath], { maxBuffer: 1 << 30, encoding: 'utf8' });
  if (r.status !== 0) {
    console.error(r.stderr);
    throw new Error('rust oracle failed');
  }
  return r.stdout
    .split('\n')
    .filter(Boolean)
    .map((l) => JSON.parse(l));
}

// Сравнение: возвращает null (совпало), 'fallback' или описание расхождения.
function compare(js, rustRaw) {
  if (rustRaw.threw && rustRaw.kind === 'Unsupported') return 'fallback';
  const rust = normRust(rustRaw);
  if (js.threw !== rust.threw)
    return `threw js=${js.threw} rust=${rust.threw} (${js.message || rust.message})`;
  if (JSON.stringify(js.calls) !== JSON.stringify(rust.calls))
    return 'exists() call sequence differs';
  if (js.threw) return null;
  if (JSON.stringify(js.out) !== JSON.stringify(rust.out))
    return 'output differs';
  if (js.warn !== rust.warn) return 'failedSelectors differ';
  return null;
}

async function check(cases, label) {
  const rust = runRust(cases);
  let mism = 0;
  let fallback = 0;
  let threw = 0;
  const failures = [];
  for (let i = 0; i < cases.length; i++) {
    const js = await runJs(cases[i].css, cases[i].seed);
    if (js.threw) threw++;
    const d = compare(js, rust[i]);
    if (d === 'fallback') fallback++;
    else if (d) {
      mism++;
      if (failures.length < 20)
        failures.push({ ...cases[i], why: d, js, rust: rust[i] });
    }
  }
  console.log(
    `${label}: ${cases.length} cases, ${mism} mismatches, ${fallback} fallbacks, ${threw} JS throws`
  );
  if (failures.length) {
    writeFileSync(
      join(OUT, `css-failures-${label}.json`),
      JSON.stringify(failures, null, 1)
    );
    for (const f of failures.slice(0, 3)) {
      console.log('---', f.why, 'seed', f.seed);
      console.log('css :', JSON.stringify(f.css).slice(0, 600));
      console.log('js  :', JSON.stringify(f.js).slice(0, 600));
      console.log('rust:', JSON.stringify(f.rust).slice(0, 600));
    }
  }
  return mism;
}

function mulberry32(a) {
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function makeGen(seed) {
  const r = mulberry32(seed);
  const int = (n) => Math.floor(r() * n);
  const pick = (a) => a[int(a.length)];
  const chance = (p) => r() < p;
  let pretty = chance(0.4);

  const WS = ['', ' ', '\n', '\n  ', '\t', '  ', ' \n\t', '\r\n', '\f'];
  const ws = () =>
    chance(0.003)
      ? pick(['\u00a0', '\u2028', '\u3000'])
      : pretty
        ? pick(WS)
        : chance(0.1)
          ? pick(WS)
          : '';
  const SIMPLE = [
    '.a',
    '.b',
    '#c',
    'div',
    'p',
    'span',
    '.c-d',
    'a.b',
    'ul li',
    'a>b',
    'a > b',
    'a+b',
    'a ~ b',
    '*',
    '[data-x]',
    '[data-x="a,b"]',
    "[title=':x']",
    '[a="{}"]',
    '[a="}"]',
    'a:hover',
    'a::before',
    '::after',
    ':before',
    ':after',
    ':root',
    'html',
    'body',
    'a:not(:hover)',
    ':is(.a, :focus)',
    ':where(.x,.y)',
    'a:nth-child(2n+1)',
    '.x\\:hover',
    '.sm\\:flex',
    '.a\\,b',
    '.w-1\\/2',
    'a:not( )',
    ':not()',
    'a::-webkit-scrollbar',
    'input::placeholder',
    'a:HOVER',
    '.\\31 0',
    '.\\31 0:hover',
    'a:has(> b)',
    ':is(,.a)',
    ':is(.a, )',
    ':is( , .a)',
    '.😀',
    '.é',
    '.\\😀',
    '.\\😀x',
    '.\\é',
    'a/**/b',
    'a /* c */',
    'li:first-child',
    '.f:focus-within',
    '.g::selection',
    'a:-moz-focusring',
    'td:nth-of-type(odd)',
    'a::before:hover',
    '::-moz-selection',
    '.k:not(.l)',
    'x:lang(en)',
    '.m:after:hover',
    'from',
    'to',
    '50%',
    ':root:has(.a)',
    'a\\::hover',
    '.q\\\\:hover',
    'svg|a',
    '&',
    '&:hover',
    '& .n',
    '.o,',
    ',.p',
    '.a ,  .b',
    'html body',
    'body.x',
    ':host',
    '::slotted(span)',
    ':is(.a,:hover)',
    '.r:not(:is(:hover, :focus))',
    'a:hover::before',
    'a::BEFORE',
    '.s::-webkit-input-placeholder',
    'a b',
    '.t ',
    'a:not(\n)',
    'a:not( )',
    ':is( ,.u)',
    '.v:',
    '.w::',
    'a:1',
    '"q,r"',
    "'q,r'",
    'a(b,c)',
    'a\\,  b',
    '.x\\',
    'a\\ ',
    '.y\\\\',
    '.\\(',
    '"a\\",b"',
    '.z ',
    'a:is(b,c)',
    '.e\\"',
    "x[y='a\\'b,c']",
  ];
  const RISKY_SEL = [
    'a:is(b',
    'a:not(b',
    'a:(',
    'a)b,c',
    '.a\\',
    "'x",
    '"y',
    '[a',
    '(b',
    '.a ',
  ];
  const sel1 = () => {
    let s = chance(0.01) ? pick(RISKY_SEL) : pick(SIMPLE);
    if (chance(0.25)) s += pick(['', ' ', '>', ' + ']) + pick(SIMPLE);
    return s;
  };
  const selector = () => {
    const n = 1 + int(chance(0.5) ? 1 : 5);
    let s = sel1();
    for (let i = 1; i < n; i++)
      s +=
        pick([
          ',',
          ', ',
          ' ,',
          ',\n',
          ' , ',
          ',/**/',
          ',,',
          ', ,',
          ',\t,',
          ',  ,  ',
        ]) + sel1();
    if (chance(0.05)) s = pick(['', ' ', '/*c*/']) + s;
    return s;
  };
  const PROPS = [
    'color',
    'background',
    'margin',
    'animation',
    'animation-name',
    'font-family',
    'src',
    'display',
    '*zoom',
    '_height',
    '--x',
    '--y-z',
    'transition',
    '$var',
    '$&a',
    'a$$b',
    '$`d',
    '-webkit-animation',
    'ANIMATION',
    'filter',
    'content',
    'grid-template-areas',
    'width',
    '-\\$',
    'a\\:b',
  ];
  const RISKY_PROPS = ["$'c", '(x)', '"p"', '[q]'];
  const ANIMS = ['fadeIn', 'spin', 'pulse', 'unused', 'Foo', 'bar-baz', 'none'];
  const VALUES = [
    'red',
    '0',
    '1px 2px',
    'red !important',
    'red!important',
    'red ! important',
    'red !IMPORTANT',
    'red !important ',
    'red/**/!important',
    '1px/**/2px',
    '1px /**/ 2px',
    '1px,/**/2px',
    'url(a.png)',
    'url( "a b" )',
    'url(data:image/png;base64,xx==)',
    'url(a\\)b.png)',
    '"</style>"',
    "'a</style>b'",
    'a, /* c */ b',
    'rgba(0,0,0,.5)',
    'calc(1px + 2px)',
    '"str;ing"',
    "'a\\'b'",
    '\\31 x',
    'important',
    '!ie',
    'progid:DXImageTransform.Microsoft.gradient(startColorstr=#fff)',
    'a b c',
    '"a" "b"',
    'var(--x, 1px)',
    'attr(x)',
    'x \\9',
    '"a\\"b"',
    '"a\\\\"',
    '"😀"',
    ' x',
    '/* only */',
    'fn( a , b )',
  ];
  const value = (prop) => {
    if (
      prop.startsWith('animation') ||
      prop === 'ANIMATION' ||
      prop === '-webkit-animation'
    ) {
      let v = pick(ANIMS);
      if (chance(0.5))
        v += pick([
          ' 1s',
          ' 2s linear infinite',
          '\n1s',
          ', ' + pick(ANIMS) + ' 1s',
        ]);
      if (chance(0.1)) v = '/*x*/' + v;
      return v;
    }
    if (prop.startsWith('--') && chance(0.3))
      return pick([
        '{a:b}',
        '{ a: b; c: d }',
        '[1,2]',
        '',
        ' ',
        chance(0.1) ? 'x;y' : 'x y',
      ]);
    return pick(VALUES);
  };
  const comment = () =>
    pick([
      '/* x */',
      '/**/',
      '/*critters:include*/',
      '/*critters:exclude*/',
      '/*critters:include start*/',
      '/*critters:include end*/',
      '/*critters:exclude start*/',
      '/*critters:exclude end*/',
      '/*! critters:include */',
      '/* critters:include */',
      '/*critters:includex*/',
      '/*critters:*/',
      '/*critters:include\nx*/',
      '/*!*/',
      '/* # sourceMappingURL= */',
    ]);
  const decl = () => {
    const p = chance(0.01) ? pick(RISKY_PROPS) : pick(PROPS);
    let d =
      ws() +
      p +
      ws() +
      pick([':', ':', ': ', ' : ', ':/**/', '/**/:']) +
      ws() +
      value(p);
    if (chance(0.01))
      d = pick([';', '; ;', 'x', 'a:b c:d', ':a', '"s"x:y', '(a)b:c', '!x:y']);
    return d;
  };
  const block = (depth) => {
    const parts = [];
    const n = int(5);
    for (let i = 0; i < n; i++) {
      const k = r();
      if (k < 0.6) parts.push(decl() + (i < n - 1 || chance(0.6) ? ';' : ''));
      else if (k < 0.7) parts.push(comment());
      else if (k < 0.85 && depth < 3) parts.push(rule(depth + 1));
      else if (depth < 3) parts.push(atrule(depth + 1));
      else parts.push(';');
      if (chance(0.05)) parts.push(';');
    }
    return parts.join(ws());
  };
  const rule = (depth) =>
    selector() +
    ws() +
    '{' +
    ws() +
    block(depth) +
    ws() +
    '}' +
    (chance(0.05) ? ';' : '');
  const kf = (name) => {
    const pre = pick([
      '@keyframes',
      '@keyframes',
      '@-webkit-keyframes',
      '@KEYFRAMES',
    ]);
    return `${pre} ${name}${ws()}{${pick(['from', '0%', 'from , to'])}{opacity:0}${ws()}${pick(['to', '100%', '50%,to'])}{opacity:1}}`;
  };
  const atrule = (depth) => {
    const k = int(16);
    switch (k) {
      case 0:
        return `@media ${pick(['(min-width:1px)', 'screen and (max-width: 2px)', 'print', 'all'])}${ws()}{${ws()}${rules(depth + 1)}${ws()}}`;
      case 1:
        return `@supports (display:grid)${ws()}{${rules(depth + 1)}}`;
      case 2:
        return `@layer ${pick(['a', 'a, b', 'x.y'])};`;
      case 3:
        return `@layer x{${rules(depth + 1)}}`;
      case 4:
        return `@container ${pick(['(min-width: 1px)', 'card (width > 2px)'])}{${rules(depth + 1)}}`;
      case 5:
        return kf(pick(ANIMS));
      case 6:
        return `@font-face{font-family:${pick(['x', '"Y"'])};src:url(a.woff2)${pick(['', ' format("woff2")'])}}`;
      case 7:
        return chance(0.15)
          ? '@font-face;'
          : pick(['@font-face{}', '@font-face{a{}}', '@font-face{/**/}']);
      case 8:
        return pick([
          '@import "a.css";',
          '@import url(b.css) screen;',
          "@charset 'utf-8';",
          '@import"c.css";',
        ]);
      case 9:
        return `@media x${ws()}{${ws()}}`;
      case 10:
        return pick([
          '@page{margin:0}',
          '@page :first{margin:0}',
          '@namespace svg url(x);',
        ]);
      case 11:
        return pick([
          '@foo bar',
          '@foo bar;',
          '@foo/*c*/;',
          '@foo (a)[b]{c:d}',
          '@foo bar ',
          '@foo{}',
          '@foo [a{]{}',
          '@foo (a;b);',
          '@foo url(x;y);',
        ]);
      case 12:
        return `@media${pick([' ', '', '/**/', ' /**/ '])}${pick(['(x)', 'screen', ''])}${pick([' ', '', '\n'])}{${rules(depth + 1)}}`;
      case 13:
        return `@media screen{${comment()}${rules(depth + 1)}}`;
      case 14:
        return `@supports not (x:y){${block(depth + 1)}}`;
      default:
        return `@media (hover:hover){${rule(depth + 1)}}`;
    }
  };
  const rules = (depth) => {
    const parts = [];
    const n = 1 + int(4);
    for (let i = 0; i < n; i++) {
      const k = r();
      if (k < 0.6 || depth >= 3) parts.push(rule(depth));
      else if (k < 0.8) parts.push(atrule(depth));
      else parts.push(comment());
    }
    return parts.join(ws());
  };
  const ERRORS = [
    (s) => s + '{',
    (s) => s + '}',
    (s) => s + '"x',
    (s) => s + '/* x',
    (s) => s + 'a{b}',
    (s) => s + '@{}',
    (s) => s + 'a{::b;}',
    (s) => s + 'a{color:red background:blue}',
    (s) => s + 'a{background:url(}',
    (s) => s + 'a{;}',
    (s) => s + ';',
    (s) => s + ';;',
    (s) => s + 'a{b:c}}',
    (s) => s + 'x',
    (s) => s + 'a{b:(}',
    (s) => s + 'a{b:[}',
    (s) => s + 'a{--x:{a:b}',
    (s) => s + 'a{b:c;d}',
    (s) => s + '\\',
    (s) => '﻿' + s,
    (s) => s + 'a{b:c:d}',
    (s) => s + 'a{b:progid:x}',
    (s) => s + 'a{filter:progid:DX(a:b)}',
    (s) => s + 'a{b:c}x:y',
    (s) => s + 'a{b: c;',
    (s) => s + '@media x{a{b:c}',
    (s) => s + '( a{b:c} )',
    (s) => s + 'a{b:"x\\"}',
    (s) => s + '/*# sourceMappingURL=a.css.map */',
    (s) => s + '/*# sourceMappingURL= */',
  ];
  return () => {
    pretty = chance(0.4);
    let css = '';
    const n = 1 + int(8);
    for (let i = 0; i < n; i++) {
      const k = r();
      if (k < 0.55) css += rule(0);
      else if (k < 0.85) css += atrule(0);
      else css += comment();
      css += ws();
    }
    if (chance(0.12)) css = pick(ERRORS)(css);
    if (chance(0.02)) css = pick(['', ' ', '\n', '/**/', ';', '﻿']);
    const seed = chance(0.08) ? -1 : chance(0.08) ? -2 : int(1000);
    return { css, seed };
  };
}

function fuzzCases(n, seed0) {
  const gen = makeGen(seed0);
  const out = [];
  for (let i = 0; i < n; i++) out.push(gen());
  return out;
}

function realCases(seeds) {
  if (!REAL_DIR) throw new Error('ORACLE_CSS_DIR is not set');
  const files = readdirSync(REAL_DIR)
    .filter((f) => f.endsWith('.css'))
    .sort();
  const cases = [];
  for (const f of files) {
    const css = readFileSync(join(REAL_DIR, f), 'utf8');
    for (const seed of seeds) cases.push({ css, seed, file: f });
  }
  return cases;
}

const [mode = 'all', ...args] = process.argv.slice(2);
if (mode === 'fuzz' || mode === 'all') {
  const n = Number(args[0] || 6000);
  const s0 = Number(args[1] || 1);
  const m = await check(fuzzCases(n, s0), `fuzz(${n},seed0=${s0})`);
  if (mode === 'fuzz') process.exit(m ? 1 : 0);
}
if (mode === 'real' || mode === 'all') {
  const m = await check(realCases([-1, -2, 1, 2, 3, 4]), 'real');
  process.exit(m ? 1 : 0);
}
if (mode === 'inputs') {
  const picked = JSON.parse(
    readFileSync(join(ROOT, args[0] || 'oracle/css-golden-inputs.json'), 'utf8')
  );
  process.exit((await check(picked, 'inputs')) ? 1 : 0);
}
if (mode === 'case') {
  const c = { css: args[0], seed: Number(args[1] ?? 1) };
  console.log('js  :', JSON.stringify(await runJs(c.css, c.seed)));
  console.log('rust:', JSON.stringify(runRust([c])[0]));
}
function rustStr(x) {
  let o = '"';
  for (const ch of x) {
    const cp = ch.codePointAt(0);
    if (ch === '"') o += '\\"';
    else if (ch === '\\') o += '\\\\';
    else if (ch === '\n') o += '\\n';
    else if (ch === '\r') o += '\\r';
    else if (ch === '\t') o += '\\t';
    else if (
      cp < 0x20 ||
      cp === 0x7f ||
      (cp >= 0x80 && cp < 0xa0) ||
      cp === 0x2028 ||
      cp === 0x2029 ||
      cp === 0xfeff ||
      cp === 0xa0 ||
      (cp >= 0x2000 && cp <= 0x200b) ||
      cp === 0x3000
    )
      o += '\\u{' + cp.toString(16) + '}';
    else o += ch;
  }
  return o + '"';
}
if (mode === 'golden') {
  // Отобранные вручную граничные входы + срез fuzz-выхода с ожиданиями от JS.
  const n = Number(args[0] || 30);
  const picked = JSON.parse(
    readFileSync(join(ROOT, 'oracle/css-golden-inputs.json'), 'utf8')
  );
  const cases = [
    ...picked,
    ...fuzzCases(400, 4242)
      .filter((c) => c.css.length < 300 && !c.css.includes('sourceMappingURL'))
      .slice(0, n),
  ];
  const lines = [];
  for (const c of cases) {
    const js = await runJs(c.css, c.seed);
    let exp;
    if (js.threw) exp = 'Expect::Throw';
    else if (js.out.skipped) exp = 'Expect::Skipped';
    else if (js.out.removed) exp = 'Expect::Removed';
    else exp = `Expect::Css(${rustStr(js.out.css)})`;
    lines.push(
      `    (${rustStr(c.css)}, ${c.seed}, ${exp}, ${rustStr(js.warn ?? '')}, &[${js.calls
        .map((x) => rustStr(x))
        .join(', ')}]),`
    );
  }
  console.log(lines.join('\n'));
}
if (mode === 'bench') {
  const cases = realCases([0]);
  for (const [label, seed] of [
    ['always-true', -1],
    ['hash-seed-1', 1],
  ]) {
    for (const c of cases) await runJs(c.css, seed);
    const iters = 10;
    const t = process.hrtime.bigint();
    for (let i = 0; i < iters; i++)
      for (const c of cases) await runJs(c.css, seed);
    const ms = Number(process.hrtime.bigint() - t) / 1e6 / iters;
    const max = cases.reduce((a, b) => (a.css.length > b.css.length ? a : b));
    const t2 = process.hrtime.bigint();
    for (let i = 0; i < iters; i++) await runJs(max.css, seed);
    const ms2 = Number(process.hrtime.bigint() - t2) / 1e6 / iters;
    console.log(
      `JS ${label}: ${cases.length} files, ${ms.toFixed(2)} ms total (${(ms / cases.length).toFixed(3)} ms/file); largest (${max.css.length}) ${ms2.toFixed(2)} ms`
    );
  }
}
