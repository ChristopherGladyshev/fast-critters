import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { clearInlineCssCache, createInlineAllCss } from '../src/core/inlineAllCss';

import type { TCrittersConstructor } from '../src/core/types';

/** Заменяет critters: по результату `process` видно, какая сторона отработала. */
class FakeCritters {
  options: Record<string, unknown>;

  constructor(options: Record<string, unknown>) {
    this.options = options;
  }

  async process(html: string) {
    return `critical:${html}`;
  }
}

const InlineAll = createInlineAllCss(
  FakeCritters as unknown as TCrittersConstructor
) as unknown as typeof FakeCritters & { isInlineAllCss: boolean };

const dist = mkdtempSync(join(tmpdir(), 'inline-all-css-'));
mkdirSync(join(dist, 'static/css'), { recursive: true });
writeFileSync(join(dist, 'static/css/app.css'), '.app{color:red}');
writeFileSync(join(dist, 'static/css/page.css'), '.page{margin:0}');
writeFileSync(join(dist, 'static/css/lazy.css'), '.lazy{content:"$&"}');
writeFileSync(join(dist, 'static/css/evil.css'), '.x{content:"</style>"}');

const process = (html: string, options = {}) =>
  new InlineAll({ path: dist, publicPath: '/_next/', ...options }).process(
    html
  );

const link = (name: string, kind = '') =>
  `<link rel="preload" href="/_next/static/css/${name}.css" as="style"/>` +
  `<link rel="stylesheet" href="/_next/static/css/${name}.css"${kind}/>`;

describe('createInlineAllCss', () => {
  beforeEach(() => clearInlineCssCache());

  it('inlines every stylesheet in place, keeping what the Next.js client looks up', async () => {
    const html =
      '<html><head><meta charset="utf-8"/>' +
      link('app', ' data-n-g=""') +
      link('page', ' data-n-p=""') +
      link('lazy') +
      '<noscript data-n-css=""></noscript></head>' +
      '<body><link rel="stylesheet" href="/_next/static/css/app.css"/></body></html>';

    await expect(process(html)).resolves.toBe(
      '<html><head><meta charset="utf-8"/>' +
        '<style data-href="/_next/static/css/app.css">.app{color:red}</style>' +
        '<style data-href="/_next/static/css/page.css" data-n-href="/_next/static/css/page.css">.page{margin:0}</style>' +
        '<style data-href="/_next/static/css/lazy.css">.lazy{content:"$&"}</style>' +
        '<noscript data-n-css=""></noscript></head>' +
        '<body><link rel="stylesheet" href="/_next/static/css/app.css"/></body></html>'
    );
    expect(InlineAll.isInlineAllCss).toBe(true);
  });

  it('keeps preloads of sheets it does not inline', async () => {
    const html =
      '<html><head><link rel="preload" href="/_next/static/css/page.css" as="style"/>' +
      link('app', ' data-n-g=""') +
      '</head><body></body></html>';

    await expect(process(html)).resolves.toBe(
      '<html><head><link rel="preload" href="/_next/static/css/page.css" as="style"/>' +
        '<style data-href="/_next/static/css/app.css">.app{color:red}</style>' +
        '</head><body></body></html>'
    );
  });

  it('leaves a page without stylesheets as it is', async () => {
    const html = '<html><head></head><body><p>x</p></body></html>';

    await expect(process(html)).resolves.toBe(html);
  });

  it.each([
    ['a missing file', link('app') + link('gone')],
    ['a sheet with </style> inside', link('evil')],
    [
      'a sheet outside the build',
      '<link rel="stylesheet" href="https://cdn.example/a.css"/>',
    ],
    [
      'a path escaping static/css',
      '<link rel="stylesheet" href="/_next/static/css/../../x.css"/>',
    ],
  ])('falls back to critical CSS on %s', async (_, links) => {
    const html = `<html><head>${links}</head><body></body></html>`;

    await expect(process(html)).resolves.toBe(`critical:${html}`);
  });

  it('falls back to critical CSS without the options Next.js passes', async () => {
    const html = `<html><head>${link('app')}</head><body></body></html>`;

    await expect(process(html, { path: undefined })).resolves.toBe(
      `critical:${html}`
    );
  });
});
