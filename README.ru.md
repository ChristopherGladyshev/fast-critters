# fast-critters

[![npm version](https://img.shields.io/npm/v/fast-critters.svg)](https://www.npmjs.com/package/fast-critters)
[![npm downloads](https://img.shields.io/npm/dm/fast-critters.svg)](https://www.npmjs.com/package/fast-critters)
[![CI](https://github.com/ChristopherGladyshev/fast-critters/actions/workflows/ci.yml/badge.svg)](https://github.com/ChristopherGladyshev/fast-critters/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/npm/l/fast-critters.svg)](LICENSE)
[![node](https://img.shields.io/node/v/fast-critters.svg)](https://nodejs.org)
[![Next.js](https://img.shields.io/badge/Next.js-14%20%7C%2015%20%7C%2016-black?logo=next.js)](https://nextjs.org)
[![Rust](https://img.shields.io/badge/core-Rust-orange?logo=rust)](native)
[![GitHub stars](https://img.shields.io/github/stars/ChristopherGladyshev/fast-critters?style=social)](https://github.com/ChristopherGladyshev/fast-critters)

[English](README.md) · **Русский** · [中文](README.zh-CN.md)

> ### 🚀 +10–20 баллов Lighthouse Performance для вашего Next.js
>
> Критический CSS встраивается в каждую страницу: стили больше не блокируют
> рендер, First Contentful Paint и LCP становятся быстрее. Пакет достаточно
> быстрый, чтобы работать на каждом SSR-запросе: **~8 мс на страницу вместо ~230 мс**.

<sub>Прирост даёт устранение блокирующего рендер CSS, поэтому конкретная цифра
зависит от ваших страниц и объёма CSS на них. Замерьте до и после на своих
страницах.</sub>

---

## Зачем

Next.js уже умеет встраивать критический CSS: `experimental.optimizeCss`
прогоняет каждую отрендеренную страницу через
[`critters`](https://github.com/GoogleChromeLabs/critters). На практике
большинство его выключает, потому что:

- **Он медленный.** Stock critters тратит около 230 мс CPU на страницу в
  главном потоке на каждом SSR-запросе. Это прямо ложится в TTFB и в счёт за
  серверы.
- **Он ломается под нагрузкой.** У critters 0.0.23 глобальный кэш классов, и
  параллельно рендерящиеся страницы портят друг другу критический CSS.

**fast-critters** исправляет обе проблемы, а вывод остаётся таким же.

## Что вы получаете

| | stock critters 0.0.23 | **fast-critters** |
| --- | --- | --- |
| Время на страницу | ~230 мс | **~8 мс** (Rust), ~40 мс (JS-фолбэк) |
| Где работает | главный поток Node | **пул потоков libuv**, event loop свободен |
| Параллельные рендеры | может перепутать критический CSS между страницами | **корректно** |
| Вывод | — | **байт-в-байт** как у stock |
| Миграция | — | один `npm i` и одна строка в `next.config.js` |

- **Rust-порт (napi-аддон).** Весь `Critters#process(html)` (HTML-парсер,
  CSS-парсер, движок селекторов) переписан на Rust и сверяется с
  оригинальными JS-библиотеками golden-тестами и фаззингом.
- **Безопасные фолбэки.** Если под платформу нет собранного аддона или в
  документе есть что-то, чего порт не поддерживает, работает **FastCritters**:
  JS-подкласс critters с индексом селекторов, в ~5,6 раза быстрее stock. Если и
  он упадёт, сработает stock critters. Страницы рендерятся всегда.
- **Работает с webpack и Turbopack**, а также с `output: 'standalone'`.

## Быстрый старт (Next.js)

Пакет ставится **под именем `critters`**, чтобы Next.js подхватил его вместо
stock:

```bash
npm i critters@npm:fast-critters
```

```js
// next.config.js
const { withFastCritters } = require('critters/next');

module.exports = withFastCritters({
  // ваш конфиг; можно и функцию (phase) => config
});
```

Это всё. `withFastCritters(config, options)`:

- включает `experimental.optimizeCss` (опции critters передаются в
  `options.critters`, Next.js добавит их к своим);
- добавляет `bin/*.node` в `outputFileTracingIncludes`, чтобы
  `output: 'standalone'` скопировал аддон, а не молча работал на JS;
- предупреждает, если `critters` резолвится в какой-то другой пакет.

При старте в лог пишется, какая реализация активна, например `critters: rust`.

> **Важно:** работает только с **Pages Router**. App Router `optimizeCss` не
> использует.

### Проверено на Next.js 16.2.2

| Сборка | `next start` | standalone | SSR против stock |
| --- | --- | --- | --- |
| Turbopack (`next build`) | `critters: rust` | `critters: rust` | байт-в-байт |
| webpack (`next build --webpack`) | `critters: rust` | `critters: rust` | байт-в-байт |

Рабочий пример лежит в [`examples/next-pages`](examples/next-pages).

## Плагин webpack (без Next.js)

Для статического HTML, который выдаёт сборка (например, `html-webpack-plugin`):

```js
const { FastCrittersPlugin } = require('fast-critters/webpack');

module.exports = {
  output: { publicPath: '/' },
  plugins: [
    new HtmlWebpackPlugin(),
    new MiniCssExtractPlugin(),
    new FastCrittersPlugin({ critters: { /* опции critters */ } }),
  ],
};
```

Плагин правит готовые файлы на диске после emit (`afterEmit`). В
`mode: development` он выключен (`isDevEnabled: true` включает). Опции по
умолчанию те же, что у Next.js (`preload: 'media'`, `fonts: false`,
`reduceInlineStyles: false`). С ними работает Rust-порт, с другими опциями
документ обрабатывает JS-реализация.

У Turbopack нет API плагинов для HTML, поэтому вне Next.js он не поддержан.

## Переменные окружения

| Переменная | Смысл |
| --- | --- |
| `CRITTERS_NATIVE_PATH` | путь к аддону вместо `bin/fast-critters.<platform>.node` |
| `CRITTERS_NATIVE=false` | kill-switch: не грузить аддон, только JS |
| `FAST_CRITTERS=false` | kill-switch всего: stock critters |
| `INLINE_ALL_CSS=true` | встраивать весь CSS страницы, а не только критический |
| `CRITTERS_LOG_LEVEL` | `info` / `warn` / `silent` для лога выбора реализации |

## Требования

- Node.js ≥ 20
- Next.js ≥ 14 (Pages Router) или webpack 5
- Готовый Rust-аддон: Linux x64/arm64 (glibc ≥ 2.17 и musl/Alpine), macOS x64/arm64, Windows x64/arm64. На остальных платформах работает JS-фолбэк.

## Поддержка

Если вы пользуетесь fast-critters, поставьте [звезду на GitHub](https://github.com/ChristopherGladyshev/fast-critters) ⭐.
Так проект проще найти другим.

## Разработка

Сборка, тесты, устройство Rust-крейтов и оракулы эквивалентности описаны в
[CONTRIBUTING.md](CONTRIBUTING.md).

## Лицензия

[MIT](LICENSE)

<sub>Keywords: `critters` · `critters-alternative` · `critters-replacement` · `beasties` · `critical-css` · `critical` · `inline-css` · `css-inline` · `inline-critical-css` · `above-the-fold` · `render-blocking` · `render-blocking-css` · `css-optimization` · `optimize-css` · `optimizeCss` · `nextjs` · `next` · `next.js` · `nextjs-plugin` · `pages-router` · `ssr` · `server-side-rendering` · `lighthouse` · `core-web-vitals` · `web-vitals` · `fcp` · `lcp` · `pagespeed` · `page-speed` · `performance` · `web-performance` · `seo` · `rust` · `napi` · `napi-rs` · `native-addon` · `webpack` · `webpack-plugin` · `html-webpack-plugin` · `turbopack` · `css` · `html` · `postcss` · `fast` · `speed`</sub>
