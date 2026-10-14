// Перегенерирует tests/fixtures/*/{expected.html,logs.json} стоковым critters 0.0.23.
// использование: node oracle/pipeline-golden.mjs
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';

const require = createRequire(import.meta.url);
const Critters = require('../../node_modules/critters-upstream');
const root = path.join(
  import.meta.dirname,
  '..',
  'crates',
  'critters',
  'tests',
  'fixtures'
);

for (const name of fs.readdirSync(root).sort()) {
  const dir = path.join(root, name);
  const html = fs.readFileSync(path.join(dir, 'input.html'), 'utf8');
  const logs = [];
  const critters = new Critters({
    ssrMode: true,
    reduceInlineStyles: false,
    path: dir,
    publicPath: '/_next/',
    preload: 'media',
    fonts: false,
    logger: {
      warn: (m) => logs.push(['warn', m]),
      info: (m) => !m.startsWith('Time ') && logs.push(['info', m]),
      error() {},
      debug() {},
      trace() {},
    },
  });
  const out = await critters.process(html);
  fs.writeFileSync(path.join(dir, 'expected.html'), out);
  fs.writeFileSync(
    path.join(dir, 'logs.json'),
    JSON.stringify(
      logs.map(([l, m]) => [l, m.split(dir).join('<DIR>')]),
      null,
      1
    ) + '\n'
  );
  console.log(name, out.length, logs.length);
}
