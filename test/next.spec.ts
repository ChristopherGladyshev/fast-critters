import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { withFastCritters } from '../src/next';

/** Приложение, у которого `critters` установлен как пакет с именем `name`. */
const app = (name: string | null) => {
  const dir = mkdtempSync(join(tmpdir(), 'fast-critters-next-'));
  if (name) {
    const pkg = join(dir, 'node_modules/critters');
    mkdirSync(pkg, { recursive: true });
    writeFileSync(join(pkg, 'package.json'), JSON.stringify({ name }));
    writeFileSync(join(pkg, 'index.js'), '');
  }
  return dir;
};

describe('withFastCritters', () => {
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  beforeEach(() => warn.mockClear());

  it('enables optimizeCss and traces the native addon', () => {
    const dir = app('fast-critters');
    const config = withFastCritters(
      { reactStrictMode: true, experimental: { scrollRestoration: true } },
      { dir }
    );

    expect(config).toEqual({
      reactStrictMode: true,
      experimental: { scrollRestoration: true, optimizeCss: true },
      outputFileTracingIncludes: { '/*': ['node_modules/critters/bin/*.node'] },
    });
    expect(warn).not.toHaveBeenCalled();
  });

  it('keeps existing tracing includes and passes critters options', () => {
    const dir = app('fast-critters');
    const config = withFastCritters(
      { outputFileTracingIncludes: { '/*': ['public/x'], '/api': ['y'] } },
      { dir, critters: { inlineThreshold: 0 } }
    );

    expect(config.experimental).toEqual({
      optimizeCss: { inlineThreshold: 0 },
    });
    expect(config.outputFileTracingIncludes).toEqual({
      '/*': ['public/x', 'node_modules/critters/bin/*.node'],
      '/api': ['y'],
    });
  });

  it('warns when critters is stock or missing', () => {
    for (const name of ['critters', null]) {
      const config = withFastCritters({}, { dir: app(name) });
      expect(config.experimental).toEqual({ optimizeCss: true });
      expect(config.outputFileTracingIncludes).toEqual({});
    }
    expect(warn).toHaveBeenCalledTimes(2);
    expect(warn.mock.calls[0][0]).toContain(
      'npm i critters@npm:fast-critters'
    );
  });

  it('wraps a config function', async () => {
    const dir = app('fast-critters');
    const config = withFastCritters(
      async (phase: unknown) => ({ env: { phase: String(phase) } }),
      { dir }
    );

    await expect(config('phase-production-build')).resolves.toMatchObject({
      env: { phase: 'phase-production-build' },
      experimental: { optimizeCss: true },
    });
  });
});
