# fast-critters

[![npm version](https://img.shields.io/npm/v/fast-critters.svg)](https://www.npmjs.com/package/fast-critters)
[![npm downloads](https://img.shields.io/npm/dm/fast-critters.svg)](https://www.npmjs.com/package/fast-critters)
[![CI](https://github.com/ChristopherGladyshev/fast-critters/actions/workflows/ci.yml/badge.svg)](https://github.com/ChristopherGladyshev/fast-critters/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/npm/l/fast-critters.svg)](LICENSE)
[![node](https://img.shields.io/node/v/fast-critters.svg)](https://nodejs.org)
[![Next.js](https://img.shields.io/badge/Next.js-14%20%7C%2015%20%7C%2016-black?logo=next.js)](https://nextjs.org)
[![Rust](https://img.shields.io/badge/core-Rust-orange?logo=rust)](native)
[![GitHub stars](https://img.shields.io/github/stars/ChristopherGladyshev/fast-critters?style=social)](https://github.com/ChristopherGladyshev/fast-critters)

**English** · [Русский](README.ru.md) · [中文](README.zh-CN.md)

> ### 🚀 +10–20 Lighthouse Performance points for your Next.js app
>
> Critical CSS inlined into every page, no render-blocking stylesheets, and a
> faster First Contentful Paint and LCP. It's fast enough to run on every SSR
> request: **~8 ms per page instead of ~230 ms**.

<sub>The gain comes from removing render-blocking CSS, so the actual number
depends on your pages and how much CSS they load. Measure before and after on
your own pages.</sub>

---

## Why

Next.js can already inline critical CSS: `experimental.optimizeCss` runs
[`critters`](https://github.com/GoogleChromeLabs/critters) on each rendered
page. In practice, most teams turn it off, because:

- **It's slow.** Stock critters takes about 230 ms of CPU per page, on the main
  thread, on every SSR request. That cost goes straight into your TTFB and
  server bill.
- **It's broken under load.** critters 0.0.23 keeps a global class cache, so
  pages rendered concurrently can corrupt each other's critical CSS.

**fast-critters** fixes both and keeps the output exactly the same.

## What you get

| | stock critters 0.0.23 | **fast-critters** |
| --- | --- | --- |
| Time per page | ~230 ms | **~8 ms** (Rust), ~40 ms (JS fallback) |
| Where it runs | Node main thread | **libuv thread pool**, the event loop stays free |
| Concurrent renders | can mix up critical CSS between pages | **correct** |
| Output | — | **byte-for-byte identical** to stock |
| Migration | — | one `npm i` and one line in `next.config.js` |

- **Rust port (napi addon).** The full `Critters#process(html)` pipeline
  (HTML parser, CSS parser, selector engine) is rewritten in Rust and checked
  against the original JS libraries with golden tests and fuzzing.
- **Safe fallbacks.** If there's no prebuilt addon for your platform, or a
  document uses something the port doesn't support, it falls back to
  **FastCritters** (an indexed JS subclass of critters, ~5.6× faster than
  stock). If that fails too, stock critters runs. Your pages always render.
- **Works with webpack and Turbopack**, and with `output: 'standalone'`.

## Quick start (Next.js)

Install the package **under the name `critters`**, so Next.js picks it up
instead of the stock one:

```bash
npm i critters@npm:fast-critters
```

```js
// next.config.js
const { withFastCritters } = require('critters/next');

module.exports = withFastCritters({
  // your config; a (phase) => config function also works
});
```

That's it. `withFastCritters(config, options)`:

- turns on `experimental.optimizeCss` (pass critters options as
  `options.critters`; Next.js adds them to its own);
- adds `bin/*.node` to `outputFileTracingIncludes`, so `output: 'standalone'`
  ships the native addon instead of silently running on JS;
- warns you if `critters` resolves to some other package.

At startup it logs which implementation is active, for example
`critters: rust`.

> **Note:** it only works with the **Pages Router**. The App Router doesn't
> use `optimizeCss`.

### Tested on Next.js 16.2.2

| Build | `next start` | standalone | SSR vs. stock |
| --- | --- | --- | --- |
| Turbopack (`next build`) | `critters: rust` | `critters: rust` | byte-for-byte |
| webpack (`next build --webpack`) | `critters: rust` | `critters: rust` | byte-for-byte |

A working example is in [`examples/next-pages`](examples/next-pages).

## webpack plugin (without Next.js)

For static HTML produced by your build (for example, with `html-webpack-plugin`):

```js
const { FastCrittersPlugin } = require('fast-critters/webpack');

module.exports = {
  output: { publicPath: '/' },
  plugins: [
    new HtmlWebpackPlugin(),
    new MiniCssExtractPlugin(),
    new FastCrittersPlugin({ critters: { /* critters options */ } }),
  ],
};
```

The plugin rewrites the emitted files on disk (`afterEmit`). It's disabled in
`mode: development` (set `isDevEnabled: true` to enable it). The defaults match
Next.js (`preload: 'media'`, `fonts: false`, `reduceInlineStyles: false`). The
Rust port handles those defaults. With other options, the JS implementation
processes the document.

Turbopack has no plugin API for HTML, so outside Next.js it isn't supported.

## Environment variables

| Variable | Effect |
| --- | --- |
| `CRITTERS_NATIVE_PATH` | path to the addon instead of `bin/fast-critters.<platform>.node` |
| `CRITTERS_NATIVE=false` | kill switch: don't load the addon, use JS only |
| `FAST_CRITTERS=false` | full kill switch: use stock critters |
| `INLINE_ALL_CSS=true` | inline all of the page's CSS, not just the critical part |
| `CRITTERS_LOG_LEVEL` | `info` / `warn` / `silent` for the implementation log |

## Requirements

- Node.js ≥ 20
- Next.js ≥ 14 (Pages Router) or webpack 5
- Prebuilt Rust addon: Linux x64/arm64 (glibc ≥ 2.17 and musl/Alpine), macOS x64/arm64, Windows x64/arm64. Other platforms use the JS fallback.

## Support

If fast-critters is useful to you, please [star it on GitHub](https://github.com/ChristopherGladyshev/fast-critters) ⭐.
It helps other people find the project.

## Contributing

Build, tests, Rust crate layout and the equivalence oracles are covered in
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE)

<sub>Keywords: `critters` · `critters-alternative` · `critters-replacement` · `beasties` · `critical-css` · `critical` · `inline-css` · `css-inline` · `inline-critical-css` · `above-the-fold` · `render-blocking` · `render-blocking-css` · `css-optimization` · `optimize-css` · `optimizeCss` · `nextjs` · `next` · `next.js` · `nextjs-plugin` · `pages-router` · `ssr` · `server-side-rendering` · `lighthouse` · `core-web-vitals` · `web-vitals` · `fcp` · `lcp` · `pagespeed` · `page-speed` · `performance` · `web-performance` · `seo` · `rust` · `napi` · `napi-rs` · `native-addon` · `webpack` · `webpack-plugin` · `html-webpack-plugin` · `turbopack` · `css` · `html` · `postcss` · `fast` · `speed`</sub>
