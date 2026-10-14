# Разработка fast-critters

Пользовательская документация — [README.md](README.md) ([RU](README.ru.md), [中文](README.zh-CN.md)).

```bash
npm i
npm run build          # TS → dist/
npm run build:native   # cargo build --release → bin/fast-critters.<platform>-<arch>[-gnu|-musl].node
npm test               # vitest: эквивалентность stock, Rust-аддон, withFastCritters, webpack
npm run typecheck
npm run test:native    # cargo test: golden-кейсы, снятые с JS
npm run lint:native    # cargo fmt --check + clippy -D warnings
```

Тесты Rust-аддона в vitest требуют `npm run build && npm run build:native`, иначе пропускаются.

Пример Next.js: `npm pack`, затем `cd examples/next-pages && npm i && npm run build`
(или `build:webpack`), `npx next start -p 3123`.

## Структура

| Путь | Что |
| --- | --- |
| `index.js` | то, что получает `require('critters')`: класс порта |
| `src/core` | FastCritters, обёртка Rust-аддона, загрузка, `INLINE_ALL_CSS` |
| `src/next.ts` | `withFastCritters` (`critters/next`) |
| `src/webpack.ts` | `FastCrittersPlugin` |
| `native/` | Rust-воркспейс аддона |
| `native/oracle` | оракулы: сравнение крейтов с JS-библиотеками на фаззе |

## Rust-крейты

| crate | порт чего |
| --- | --- |
| `crates/dom` | domhandler 5 + мутации domutils 3 (арена узлов) |
| `crates/html` | htmlparser2 8.0.2 (`decodeEntities:false`) + dom-serializer 2.0.0 |
| `crates/select` | css-what 6.2.2 + css-select 5.2.2 + nth-check 2.1.1 |
| `crates/css` | postcss 8.5.8 (tokenizer/parser/`list.comma`) + critters `processStyle` / `serializeStylesheet` |
| `crates/critters` | critters `process()`: контейнер, `<link>` → `<style>`, `media=print`+`noscript`, слияние, логи; индекс tag/class/id для `exists()` |
| корень `native/` | napi: `process(html, {path, publicPath})` (AsyncTask) |

Всё, что порт не поддерживает или на чём JS бросает исключение, возвращается
как `fallback`, и документ обрабатывает JS.

## Оракулы

Любая правка порта должна сохранять вывод байт-в-байт со stock critters 0.0.23.
Из `native/`, нужен `node_modules` пакета:

```bash
node oracle/html.mjs                 # htmlparser2 + dom-serializer (собирает себя сам)
CARGO_TARGET_DIR=target-select cargo build --release -p select --example oracle
node oracle/select.mjs fuzz          # css-what + css-select
CARGO_TARGET_DIR=target-css cargo build --release -p css --example oracle
node oracle/css.mjs fuzz             # postcss + critters processStyle
node oracle/pipeline-golden.mjs      # пересобрать crates/critters/tests/fixtures из stock critters
```

Режимы на реальных данных: `ORACLE_CSS_DIR=<app>/.next/static/css` для
`css.mjs real` и `select.mjs real`, `ORACLE_PAGES_DIR=<dir>` для `html.mjs pages`,
`SELECT_PAGES=<dir>` для `select.mjs real`.

Версии JS-библиотек закреплены тестами: при обновлении critters / postcss /
css-select / htmlparser2 пересобрать golden-файлы оракулами и прогнать фазз.

## CI и публикация

`.github/workflows/ci.yml`: проверки, сборка аддона под 8 платформ
(linux через `cargo zigbuild`, glibc 2.17), тесты на каждой платформе с
`CRITTERS_REQUIRE_ADDON=true` (без аддона тесты падают, а не пропускаются).

Публикация — по тегу `v*` через npm trusted publishing (OIDC, без токена):

```bash
npm version patch        # 0.1.0 → 0.1.1, коммит и тег
git push --follow-tags   # CI собирает, тестирует и публикует
```

Кросс-сборка локально: `node scripts/build-native.mjs --target <triple> [--zig]`.
