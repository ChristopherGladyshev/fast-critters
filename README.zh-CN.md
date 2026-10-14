# fast-critters

[English](README.md) · [Русский](README.ru.md) · **中文**

> ### 🚀 为你的 Next.js 应用提升 10–20 分 Lighthouse Performance
>
> 关键 CSS 内联到每个页面，样式表不再阻塞渲染，First Contentful Paint 和 LCP
> 更快。速度足以在每次 SSR 请求时运行：**每页约 8 ms，而原版约 230 ms**。

<sub>提升来自消除阻塞渲染的 CSS，具体分数取决于你的页面及其加载的 CSS
体积。请在自己的页面上对比前后结果。</sub>

---

## 为什么需要它

Next.js 本身就能内联关键 CSS：`experimental.optimizeCss` 会对每个渲染出的页面
运行 [`critters`](https://github.com/GoogleChromeLabs/critters)。但实际上大多数
团队都把它关掉了，因为：

- **太慢。** 原版 critters 每页在主线程上消耗约 230 ms CPU，而且每次 SSR 请求
  都要执行。这些时间直接计入 TTFB 和服务器账单。
- **高并发下会出错。** critters 0.0.23 使用全局的类名缓存，并发渲染的页面会
  互相污染彼此的关键 CSS。

**fast-critters** 解决了这两个问题，输出结果保持完全一致。

## 你能得到什么

| | 原版 critters 0.0.23 | **fast-critters** |
| --- | --- | --- |
| 每页耗时 | 约 230 ms | **约 8 ms**（Rust），约 40 ms（JS 回退） |
| 运行位置 | Node 主线程 | **libuv 线程池**，不阻塞事件循环 |
| 并发渲染 | 页面之间的关键 CSS 可能串掉 | **结果正确** |
| 输出 | — | 与原版**逐字节一致** |
| 迁移成本 | — | 一次 `npm i`，`next.config.js` 里加一行 |

- **Rust 移植（napi 原生模块）。** 完整的 `Critters#process(html)` 流程（HTML
  解析器、CSS 解析器、选择器引擎）都用 Rust 重写，并通过 golden 测试和模糊测试
  与原始 JS 库逐一比对。
- **可靠的回退机制。** 如果你的平台没有预编译的原生模块，或者文档中有移植版
  尚不支持的内容，会自动使用 **FastCritters**：带选择器索引的 critters JS
  子类，比原版快约 5.6 倍。如果它也失败，则回退到原版 critters。页面始终能
  正常渲染。
- **支持 webpack 和 Turbopack**，也支持 `output: 'standalone'`。

## 快速开始（Next.js）

以 **`critters` 这个名字**安装本包，这样 Next.js 会用它替代原版：

```bash
npm i critters@npm:fast-critters
```

```js
// next.config.js
const { withFastCritters } = require('critters/next');

module.exports = withFastCritters({
  // 你的配置；也可以传入 (phase) => config 函数
});
```

就这么简单。`withFastCritters(config, options)` 会：

- 开启 `experimental.optimizeCss`（critters 选项通过 `options.critters` 传入，
  Next.js 会把它们合并到自己的选项中）；
- 把 `bin/*.node` 加入 `outputFileTracingIncludes`，确保 `output: 'standalone'`
  打包原生模块，而不是悄悄退回 JS；
- 如果 `critters` 解析到了其他包，会给出警告。

启动时会在日志中输出当前使用的实现，例如 `critters: rust`。

> **注意：** 仅适用于 **Pages Router**。App Router 不使用 `optimizeCss`。

### 已在 Next.js 16.2.2 上验证

| 构建方式 | `next start` | standalone | SSR 与原版对比 |
| --- | --- | --- | --- |
| Turbopack（`next build`） | `critters: rust` | `critters: rust` | 逐字节一致 |
| webpack（`next build --webpack`） | `critters: rust` | `critters: rust` | 逐字节一致 |

可运行的示例见 [`examples/next-pages`](examples/next-pages)。

## webpack 插件（不使用 Next.js）

用于构建产出的静态 HTML（例如 `html-webpack-plugin` 生成的页面）：

```js
const { FastCrittersPlugin } = require('fast-critters/webpack');

module.exports = {
  output: { publicPath: '/' },
  plugins: [
    new HtmlWebpackPlugin(),
    new MiniCssExtractPlugin(),
    new FastCrittersPlugin({ critters: { /* critters 选项 */ } }),
  ],
};
```

插件在 emit 之后（`afterEmit`）直接改写磁盘上的文件。在 `mode: development`
下默认关闭（设置 `isDevEnabled: true` 可开启）。默认选项与 Next.js 相同
（`preload: 'media'`、`fonts: false`、`reduceInlineStyles: false`），这些默认值
由 Rust 移植处理。使用其他选项时，文档由 JS 实现处理。

Turbopack 没有处理 HTML 的插件 API，因此在 Next.js 之外不支持 Turbopack。

## 环境变量

| 变量 | 作用 |
| --- | --- |
| `CRITTERS_NATIVE_PATH` | 原生模块路径，替代 `bin/fast-critters.<platform>.node` |
| `CRITTERS_NATIVE=false` | 开关：不加载原生模块，只用 JS |
| `FAST_CRITTERS=false` | 总开关：使用原版 critters |
| `INLINE_ALL_CSS=true` | 内联页面的全部 CSS，而不仅是关键 CSS |
| `CRITTERS_LOG_LEVEL` | 实现选择日志的级别：`info` / `warn` / `silent` |

## 环境要求

- Node.js ≥ 20
- Next.js ≥ 14（Pages Router）或 webpack 5
- 预编译 Rust 原生模块：Linux x64/arm64（glibc ≥ 2.17 及 musl/Alpine）、macOS x64/arm64、Windows x64/arm64。其他平台使用 JS 回退。

## 参与开发

构建、测试、Rust crate 结构以及等价性校验（oracle）请参阅
[CONTRIBUTING.md](CONTRIBUTING.md)（俄语）。

## 许可证

[MIT](LICENSE)
