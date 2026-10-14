import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { empty } from './empty';

import type { TCrittersConstructor } from './types';

/** `<link rel="stylesheet">` ровно в том виде, как его рендерит next/dist/pages/_document. */
const STYLESHEET =
  /<link rel="stylesheet" href="([^"]+)"( data-n-g=""| data-n-p="")?\/>/g;
const PRELOAD = /<link rel="preload" href="([^"]+)" as="style"\/>/g;
const CSS_FILE = /^static\/css\/[\w.-]+\.css$/;

/** Результат сборки не меняется, пока жив процесс. */
const files = new Map<string, Promise<string | null>>();

const readCss = (file: string) => {
  let css = files.get(file);
  if (!css) {
    css = readFile(file, 'utf8').then(
      (text) => (/<\/style/i.test(text) ? null : text),
      () => null
    );
    files.set(file, css);
  }
  return css;
};

/** Для тестов: кэш живёт дольше каталога с фикстурами. */
export const clearInlineCssCache = () => files.clear();

/**
 * Теги сохраняют то, что ждёт от них клиент Next.js:
 * - `data-href` — загрузчик CSS-чанков webpack видит, что стиль уже есть;
 * - `data-n-href` у стилей страницы (`data-n-p`) — роутер переключает их при
 *   клиентских переходах, как и стили, которые загружает сам.
 */
export function createInlineAllCss<T extends TCrittersConstructor>(Base: T): T {
  class InlineAllCss extends Base {
    static readonly isInlineAllCss = true;

    override async process(html: string): Promise<string> {
      const { path, publicPath } = this.options;
      const headEnd = html.indexOf('</head>');
      if (
        typeof path !== 'string' ||
        typeof publicPath !== 'string' ||
        headEnd < 0
      ) {
        return super.process(html);
      }

      const head = html.slice(0, headEnd);
      const entries: [string, string][] = [];
      for (const [, href] of head.matchAll(STYLESHEET)) {
        const file = href.startsWith(publicPath)
          ? href.slice(publicPath.length)
          : '';
        const css = CSS_FILE.test(file)
          ? await readCss(join(path, file))
          : null;
        if (css === null) {
          return super.process(html);
        }
        entries.push([href, css]);
      }
      if (empty(entries)) {
        return html;
      }

      const sheets = new Map(entries);
      const inlined = head
        .replace(PRELOAD, (tag, href: string) => (sheets.has(href) ? '' : tag))
        .replace(STYLESHEET, (_, href: string, kind?: string) => {
          const page = kind === ' data-n-p=""' ? ` data-n-href="${href}"` : '';
          return `<style data-href="${href}"${page}>${sheets.get(href)}</style>`;
        });
      return inlined + html.slice(headEnd);
    }
  }

  return InlineAllCss;
}
