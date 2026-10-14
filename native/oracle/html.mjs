// Оракул для crates/html: parseDocument из htmlparser2 + render из dom-serializer
// ({ decodeEntities: false }) против Rust-порта (crates/html/examples/oracle.rs).
//
// Запуск из native/:
//   node oracle/html.mjs                      # fuzz (seeds по умолчанию) + реальные страницы
//   node oracle/html.mjs fuzz [seeds] [n]     # напр. fuzz 1-20 400
//   node oracle/html.mjs pages <files...>
//   node oracle/html.mjs case '<p>x'          # вывести результат JS + Rust для одного входа
//   node oracle/html.mjs bench <file> [iters] # замер времени parse/render в JS
//   node oracle/html.mjs golden <file.json>   # входы (JSON-массив) -> пары [input, js output]
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const require = createRequire(
  new URL('../../node_modules/critters-upstream/package.json', import.meta.url)
);
const { parseDocument } = require('htmlparser2');
const render = require('dom-serializer').default;

const ROOT = new URL('..', import.meta.url).pathname;
const TARGET = process.env.CARGO_TARGET_DIR || join(ROOT, 'target-html');
const BIN = join(TARGET, 'release/examples/oracle');
// Сохранённые страницы p<N>.html для `pages` без аргументов.
const PAGES_DIR = process.env.ORACLE_PAGES_DIR;

function dump(node) {
  switch (node.type) {
    case 'root':
      return { t: 'root', c: node.children.map(dump) };
    case 'tag':
    case 'script':
    case 'style':
      return {
        t: node.type,
        n: node.name,
        a: Object.keys(node.attribs).map((k) => [k, node.attribs[k]]),
        c: node.children.map(dump),
      };
    case 'text':
      return { t: 'text', d: node.data };
    case 'comment':
      return { t: 'comment', d: node.data };
    case 'directive':
      return { t: 'directive', n: node.name, d: node.data };
    case 'cdata':
      return { t: 'cdata', c: node.children.map(dump) };
    default:
      throw new Error(`unknown node type ${node.type}`);
  }
}

function checkParents(node) {
  for (const child of node.children || []) {
    if (child.parent !== node) throw new Error('JS parent mismatch');
    checkParents(child);
  }
}

export function jsRun(html) {
  const doc = parseDocument(html, { decodeEntities: false });
  checkParents(doc);
  const tree = JSON.stringify(dump(doc)); // до render: render переименовывает foreign-элементы
  const out = render(doc, { decodeEntities: false });
  return { out, tree };
}

function build() {
  const r = spawnSync(
    'cargo',
    ['build', '-q', '--release', '-p', 'html', '--example', 'oracle'],
    {
      cwd: ROOT,
      env: { ...process.env, CARGO_TARGET_DIR: TARGET },
      stdio: 'inherit',
    }
  );
  if (r.status !== 0) process.exit(1);
}

function rustRun(inputs) {
  const input =
    inputs.map((html) => JSON.stringify({ html })).join('\n') + '\n';
  const r = spawnSync(BIN, [], { input, maxBuffer: 1 << 30 });
  if (r.status !== 0) {
    process.stderr.write(r.stderr);
    throw new Error(`rust oracle exited ${r.status}`);
  }
  const lines = r.stdout.toString('utf8').split('\n').filter(Boolean);
  if (lines.length !== inputs.length)
    throw new Error(`expected ${inputs.length} lines, got ${lines.length}`);
  return lines.map((line) => {
    const v = JSON.parse(line);
    return { out: v.out, tree: JSON.stringify(v.tree) };
  });
}

// Во что превращается JS-строка при записи в UTF-8 (одиночные суррогаты -> U+FFFD).
function utf8(s) {
  return Buffer.from(s, 'utf8').toString('utf8');
}

function utf8Tree(tree) {
  return JSON.stringify(
    JSON.parse(tree, (_, v) => (typeof v === 'string' ? utf8(v) : v))
  );
}

function firstDiff(a, b) {
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  const from = Math.max(0, i - 60);
  return `@${i}\n  js:   ${JSON.stringify(a.slice(from, i + 80))}\n  rust: ${JSON.stringify(b.slice(from, i + 80))}`;
}

function compare(label, inputs, { verbose = 3 } = {}) {
  const rust = rustRun(inputs);
  let bad = 0;
  let surrogateOnly = 0;
  const failures = [];
  inputs.forEach((html, i) => {
    const js = jsRun(html);
    // Порт выдаёт U+FFFD вместо одиночного суррогата, поэтому сравниваем байты.
    const exactOut = js.out === rust[i].out;
    const exactTree = js.tree === rust[i].tree;
    const outOk = exactOut || utf8(js.out) === rust[i].out;
    const treeOk = exactTree || utf8Tree(js.tree) === rust[i].tree;
    if (outOk && treeOk && !(exactOut && exactTree)) surrogateOnly++;
    if (!outOk || !treeOk) {
      bad++;
      failures.push(html);
      if (bad <= verbose) {
        console.log(
          `MISMATCH ${label}#${i} (${outOk ? '' : 'out '}${treeOk ? '' : 'tree'})`
        );
        if (html.length < 400) console.log('  input:', JSON.stringify(html));
        if (!outOk) console.log(' out', firstDiff(js.out, rust[i].out));
        if (!treeOk) console.log(' tree', firstDiff(js.tree, rust[i].tree));
      }
    }
  });
  if (failures.length)
    writeFileSync(
      join(ROOT, 'oracle/html-failures.json'),
      JSON.stringify(failures, null, 1)
    );
  console.log(
    `${label}: ${inputs.length} cases, ${bad} mismatches` +
      (surrogateOnly
        ? ` (${surrogateOnly} equal only as UTF-8: lone surrogate -> U+FFFD)`
        : '')
  );
  return bad;
}

function mulberry32(seed) {
  return function () {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const TAGS = (
  'p li dd dt option optgroup tr td th table tbody thead tfoot select svg math foreignObject foreignobject ' +
  'title textarea script style noscript template br img input hr meta link div span a b i ul ol dl h1 h2 h6 ' +
  'body head html mi mo mtext desc annotation-xml clipPath linearGradient path g rt rp button form xmp iframe ' +
  'output datalist section article pre address wbr col area source s sc scr st sty t ti tit titlex scriptx ' +
  'stylex font center x-custom my:tag svg:rect p2 feGaussianBlur textPath'
).split(' ');
const ATTR_NAMES = (
  'id class href src style data-x data-Y title type rel media onload 0 1 2 10 01 99 4294967295 4294967294 ' +
  '__proto__ constructor toString viewBox preserveAspectRatio xlink:href xmlns definitionURL ' +
  'attributeName CLASS ID Href a b c x:y'
).split(' ');
const VALUES = [
  '',
  'x',
  'a b',
  "it's",
  'say "hi"',
  '&amp;',
  '&quot;',
  '&lt;b&gt;',
  'a&b',
  '&#39;',
  'привет',
  '😀',
  '/path/to?x=1&y=2',
  '>',
  '<',
  '\n',
  ' \t ',
  'a\r\nb',
  '\u0000',
  '=',
  'javascript:x',
  '0 0 24 24',
];
const TEXTS = [
  'hello',
  ' ',
  '\n',
  '\r\n',
  '\t',
  'a < b',
  'a > b',
  '<',
  '>',
  '&amp;',
  '&lt;',
  '&nbsp;',
  '&#x27;',
  '&#39',
  '&notit;',
  '&',
  'привет мир',
  '😀🎉',
  '日本語',
  '\u0000',
  ' ',
  ' ',
  '\x1c',
  '\x0f',
  '"quoted"',
  "'single'",
  '</',
  '<!',
  '<?',
  '< p>',
  '<1>',
  '<>',
  '</>',
  '</ >',
  '</1>',
  '</-->',
  '<<',
  '<a<b',
  '\x1c\x0fscript>',
];
const RAW = [
  '<!-- comment -->',
  '<!---->',
  '<!-->',
  '<!--->',
  '<!-- a -- b --->',
  '<!-- -- -->',
  '<!--x--!>',
  '<!-x>',
  '<!x>',
  '<!>',
  '<!-->-->',
  '<!---->-->',
  '<!--<!-- nested -->-->',
  '<!DOCTYPE html>',
  '<!doctype HTML PUBLIC "-//W3C//DTD HTML 4.01//EN">',
  '<!DocType\tfoo/bar>',
  '<!DOCTYPE>',
  '<!DOCTYPE/x>',
  '<!DOCTYPE x>',
  '<?xml version="1.0"?>',
  '<?PHP echo 1 ?>',
  '<?>',
  '<?Xml/x?>',
  '<![CDATA[ x < y ]]>',
  '<![CDATA[]]>',
  '<![CDATA[x]>y]]]>',
  '<![cdata[x]]>',
  '<![CDA',
  '<![CDATAX[x]]>',
  '<![other]>',
  '<script>if (a<b && c>d) x="</scr"+"ipt>";</script>',
  '<script>a</SCRIPT >',
  '<script>x</script\n>',
  '<script>x</script/>',
  '<script>x</scriptx>y</script>',
  '<script><!--</script>',
  '<script type="text/template"><div>x</div></script>',
  '<script/>after',
  '<script />after</script>',
  '<SCRIPT>X</script>',
  '<style>a>b{color:red}</style>',
  '<style>a</STYLE\t>',
  '<style/>',
  '<style>x<</style>',
  '<title>a<b>c</b></title>',
  '<title>a &amp; b</TITLE>',
  '<title/>t',
  '<textarea><b>x</b></textarea>',
  '<xmp><b></xmp>',
  '<noscript><link rel=stylesheet href=a.css></noscript>',
  '<template><p>x</template>',
  '</br>',
  '</br/>',
  '</BR >',
  '</p>',
  '</P class=x>',
  '</img>',
  '</input>',
  '</svg>',
  '</foreignObject>',
  '</title>',
  '</math>',
  '<svg viewBox="0 0 1 1"><path d="M0"/><g><rect/></g></svg>',
  '<svg><foreignObject><div/><p>x</p></foreignObject><circle r=1 /></svg>',
  '<svg><title/><desc>d</desc><linearGradient gradientUnits="x"/></svg>',
  '<math><mi/><mtext><b/></mtext><annotation-xml/></math>',
  '<svg><clippath clippathunits="a"></clippath></svg>',
  '<SVG VIEWBOX="0 0 2 2"><FEGAUSSIANBLUR STDDEVIATION=2></SVG>',
  '<table><tr><td>1<td>2<tr><td>3</table>',
  '<ul><li>a<li>b</ul>',
  '<dl><dt>a<dd>b<dt>c</dl>',
  '<select><option>a<option>b<optgroup><option>c</select>',
  '<p>a<div>b</div>',
  '<p>a<h1>b</h1>',
  '<ruby>x<rt>a<rp>b</ruby>',
  '<input><select><input>',
  '<html><head><link rel=x><script></script><body>x',
  '<a href=x/>',
  '<a href="x"/>',
  '<a b/c>',
  '<a =x>',
  '<a ==x>',
  '<a b==c>',
  '<a b= c>',
  '<a b =c>',
  '<a "b"=c>',
  "<a 'b'>",
  '<a b="c"d="e">',
  '<a b=c"d>',
  '<a/b>',
  '<a / b>',
  '<a//>',
  '<a b c d e f>',
  '<a id=1 ID=2 Id=3>',
  '<a 2=x 1=y b=z 0=w>',
  '<a __proto__=x constructor=y>',
  '<br/>',
  '<br>',
  '<br></br>',
  '<img src=x>',
  '<img/>',
  '<hr>',
  '<A HREF=X>Y</A>',
  '<dIv ClAsS=Q>z</DiV>',
  '<ÿ>',
  '<aé b=1>',
  '<a\u0000b>',
  '<a b\u0000=1>',
  '<span>\u0000</span>',
];

function pick(rnd, arr) {
  return arr[Math.floor(rnd() * arr.length)];
}

function randomCase(rnd, s) {
  const r = rnd();
  if (r < 0.3) return s.toUpperCase();
  if (r < 0.4) return s.charAt(0).toUpperCase() + s.slice(1);
  return s;
}

function attr(rnd) {
  const name = randomCase(rnd, pick(rnd, ATTR_NAMES));
  const form = rnd();
  const value = pick(rnd, VALUES);
  if (form < 0.15) return name;
  if (form < 0.3) return `${name}=${value.replace(/[\s>]/g, '') || 'v'}`;
  if (form < 0.6) return `${name}="${value.replace(/"/g, "'")}"`;
  if (form < 0.75) return `${name}='${value.replace(/'/g, '"')}'`;
  if (form < 0.8) return `${name} = "${value}"`;
  if (form < 0.85) return `${name}="${value}`; // незакрытое: поглощает до следующей кавычки
  return `${name}=${pick(rnd, ['"a"', "'b'", 'c', '""', "''"])}`;
}

function openTag(rnd) {
  const name = randomCase(rnd, pick(rnd, TAGS));
  const n = Math.floor(rnd() * rnd() * 5);
  let s = `<${name}`;
  for (let i = 0; i < n; i++)
    s += pick(rnd, [' ', ' ', '\n', '\t', '  ', '/']) + attr(rnd);
  const end = rnd();
  if (end < 0.7) s += '>';
  else if (end < 0.85) s += '/>';
  else if (end < 0.9) s += ' />';
  else if (end < 0.95) s += ' / >';
  else s += pick(rnd, ['', ' ', '/', ' x']); // незакрытое
  return s;
}

function closeTag(rnd) {
  const name = randomCase(rnd, pick(rnd, TAGS));
  return pick(rnd, [
    `</${name}>`,
    `</${name}>`,
    `</${name}>`,
    `</${name} >`,
    `</${name} junk="x">`,
    `</ ${name}>`,
    `</${name}`,
  ]);
}

const SOUP = '<>/!-?="\' \n\tabcpPsStT&;[]CDAT1é😀\u0000';

function soup(rnd) {
  const chars = Array.from(SOUP);
  let s = '';
  const n = 1 + Math.floor(rnd() * 12);
  for (let i = 0; i < n; i++) s += pick(rnd, chars);
  return s;
}

function fragment(rnd) {
  const r = rnd();
  if (r < 0.3) return openTag(rnd);
  if (r < 0.48) return closeTag(rnd);
  if (r < 0.65) return pick(rnd, TEXTS);
  if (r < 0.85) return pick(rnd, RAW);
  return soup(rnd);
}

// Случайные фрагменты конструкций: неполные теги, комментарии, атрибуты, завершающие последовательности.
function chaosCase(rnd) {
  const n = 1 + Math.floor(rnd() * 25);
  let s = '';
  for (let i = 0; i < n; i++) {
    const cps = Array.from(fragment(rnd));
    const a = Math.floor(rnd() * cps.length);
    const b = a + 1 + Math.floor(rnd() * cps.length);
    s += rnd() < 0.5 ? cps.join('') : cps.slice(a, b).join('');
  }
  return s;
}

export function genCase(rnd) {
  if (rnd() < 0.4) return chaosCase(rnd);
  const n = 1 + Math.floor(rnd() * 30);
  let s = '';
  for (let i = 0; i < n; i++) s += fragment(rnd);
  if (rnd() < 0.25) {
    const cps = Array.from(s);
    s = cps.slice(0, Math.floor(rnd() * (cps.length + 1))).join('');
  }
  return s;
}

function parseSeeds(spec) {
  const [a, b] = spec.split('-').map(Number);
  const out = [];
  for (let i = a; i <= (b ?? a); i++) out.push(i);
  return out;
}

function fuzz(seedSpec = '1-10', perSeed = 500) {
  let total = 0;
  let bad = 0;
  for (const seed of parseSeeds(seedSpec)) {
    const rnd = mulberry32(seed);
    const inputs = [];
    for (let i = 0; i < perSeed; i++) inputs.push(genCase(rnd));
    bad += compare(`fuzz seed ${seed}`, inputs, { verbose: 2 });
    total += inputs.length;
  }
  console.log(`FUZZ TOTAL: ${total} cases, ${bad} mismatches`);
  return bad;
}

function pages(files) {
  if (!files.length) {
    if (!PAGES_DIR) {
      throw new Error('pass page files or set ORACLE_PAGES_DIR');
    }
    files = readdirSync(PAGES_DIR)
      .filter((f) => /^p\d+\.html$/.test(f))
      .sort()
      .map((f) => join(PAGES_DIR, f));
  }
  const inputs = files.map((f) => readFileSync(f, 'utf8'));
  const bad = compare('real pages', inputs, { verbose: 5 });
  console.log(`PAGES: ${inputs.length - bad}/${inputs.length} identical`);
  return bad;
}

function benchJs(file, iters = 50) {
  const src = readFileSync(file, 'utf8');
  for (let i = 0; i < 10; i++)
    render(parseDocument(src, { decodeEntities: false }), {
      decodeEntities: false,
    });
  let pBest = Infinity;
  let rBest = Infinity;
  let pSum = 0;
  let rSum = 0;
  for (let i = 0; i < iters; i++) {
    let t = performance.now();
    const doc = parseDocument(src, { decodeEntities: false });
    const p = performance.now() - t;
    t = performance.now();
    render(doc, { decodeEntities: false });
    const r = performance.now() - t;
    pBest = Math.min(pBest, p);
    rBest = Math.min(rBest, r);
    pSum += p;
    rSum += r;
  }
  console.log(
    `JS ${file}: parse best ${pBest.toFixed(3)} ms avg ${(pSum / iters).toFixed(3)} ms; render best ${rBest.toFixed(3)} ms avg ${(rSum / iters).toFixed(3)} ms`
  );
}

const [cmd, ...args] = process.argv.slice(2);
if (cmd === 'bench') {
  benchJs(args[0], Number(args[1] || 50));
} else if (cmd === 'golden') {
  const inputs = JSON.parse(readFileSync(args[0], 'utf8'));
  console.log(
    JSON.stringify(
      inputs.map((html) => [html, jsRun(html).out]),
      null,
      1
    )
  );
} else {
  if (!existsSync(BIN) || process.env.HTML_ORACLE_BUILD !== '0') build();
  if (cmd === 'case') {
    const html = args.join(' ');
    const js = jsRun(html);
    const [rust] = rustRun([html]);
    console.log(
      'js:  ',
      JSON.stringify(js.out),
      '\nrust:',
      JSON.stringify(rust.out)
    );
    console.log(
      js.tree === rust.tree
        ? 'trees equal'
        : `js tree:   ${js.tree}\nrust tree: ${rust.tree}`
    );
  } else if (cmd === 'fuzz') {
    process.exitCode = fuzz(args[0], Number(args[1] || 500)) ? 1 : 0;
  } else if (cmd === 'pages') {
    process.exitCode = pages(args) ? 1 : 0;
  } else {
    const bad = fuzz() + (PAGES_DIR ? pages([]) : 0);
    process.exitCode = bad ? 1 : 0;
  }
}
