import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import HtmlWebpackPlugin from 'html-webpack-plugin';
import MiniCssExtractPlugin from 'mini-css-extract-plugin';
import webpack from 'webpack';

import { FastCrittersPlugin } from '../src/webpack';

import type { IFastCrittersPluginOptions } from '../src/webpack';

const CSS =
  '.used{color:red}.unused{color:blue}@media (min-width:1px){.used>p{margin:0}}';
const TEMPLATE =
  '<!DOCTYPE html><html><head><title>x</title></head>' +
  '<body><div class="used"><p>x</p></div></body></html>';

/** Сборка webpack: JS импортирует CSS, html-webpack-plugin вставляет `<link>`. */
const build = async (
  mode: 'production' | 'development',
  options?: IFastCrittersPluginOptions
) => {
  const dir = mkdtempSync(join(tmpdir(), 'fast-critters-webpack-'));
  writeFileSync(join(dir, 'style.css'), CSS);
  writeFileSync(join(dir, 'index.js'), "import './style.css';");
  const output = join(dir, 'dist');

  const compiler = webpack({
    mode,
    context: dir,
    entry: './index.js',
    output: { path: output, publicPath: '/' },
    module: {
      rules: [
        {
          test: /\.css$/,
          use: [MiniCssExtractPlugin.loader, require.resolve('css-loader')],
        },
      ],
    },
    optimization: { minimize: false },
    plugins: [
      new MiniCssExtractPlugin(),
      new HtmlWebpackPlugin({ templateContent: TEMPLATE, minify: false }),
      new FastCrittersPlugin(options),
    ],
  });
  const stats = await new Promise<webpack.Stats>((resolve, reject) =>
    compiler.run((error, result) => (error ? reject(error) : resolve(result!)))
  );
  await new Promise((resolve) => compiler.close(resolve));

  expect(stats.compilation.errors).toEqual([]);
  return readFileSync(join(output, 'index.html'), 'utf8');
};

describe('FastCrittersPlugin', () => {
  it('inlines critical CSS into emitted HTML', async () => {
    const html = await build('production');

    expect(html).toMatch(/<style>.used\{color:red\}.*\.used>p\{margin:0\}/);
    expect(html).not.toMatch(/<style>[^<]*\.unused/);
    expect(html).toContain('<link href="/main.css" rel="stylesheet" media="print"');
  });

  it('skips development builds unless enabled', async () => {
    expect(await build('development')).not.toContain('<style>');
    expect(await build('development', { isDevEnabled: true })).toContain(
      '<style>'
    );
  });
});
