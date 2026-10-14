import type { Mock } from 'vitest';
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { createRustCritters } from '../src/core/createRustCritters';
import { loadCritters } from '../src/core/loadCritters';
import { loadNative } from '../src/core/loadFastCritters';
import { findBundledBinary } from '../src/core/native';

import type {
  INativeCritters,
  INativeResult,
  TCrittersConstructor,
} from '../src/core/types';

const NEXT_OPTIONS = {
  ssrMode: true,
  reduceInlineStyles: false,
  path: '/app/.next',
  publicPath: '/_next/',
  preload: 'media',
  fonts: false,
  logLevel: 'warn',
};

const logger = { info: vi.fn(), warn: vi.fn() };

/** Заменяет critters: по результату `process` видно, какая сторона отработала. */
class FakeCritters {
  options: Record<string, unknown>;
  logger = logger;

  constructor(options: Record<string, unknown>) {
    this.options = {
      logLevel: 'info',
      path: '',
      publicPath: '',
      reduceInlineStyles: true,
      pruneSource: false,
      additionalStylesheets: [],
      allowRules: [],
      ...options,
    };
  }

  async process(html: string) {
    return `js:${html}`;
  }
}

const nativeReturning = (result: INativeResult) => {
  const native: INativeCritters & { process: Mock } = {
    process: vi.fn().mockResolvedValue(result),
  };
  return native;
};

const create = (native: INativeCritters) =>
  createRustCritters(
    FakeCritters as unknown as TCrittersConstructor,
    native
  ) as unknown as new (options: Record<string, unknown>) => FakeCritters;

describe('createRustCritters', () => {
  beforeEach(() => {
    logger.info.mockClear();
    logger.warn.mockClear();
  });

  it('processes documents natively with the options Next.js passes', async () => {
    const native = nativeReturning({
      html: 'rust',
      logs: [
        { level: 'warn', message: '1 rules skipped' },
        { level: 'info', message: 'Inlined 1 kB' },
      ],
    });
    const Rust = create(native);

    await expect(new Rust(NEXT_OPTIONS).process('<html>')).resolves.toBe(
      'rust'
    );
    expect(native.process).toHaveBeenCalledWith('<html>', {
      path: '/app/.next',
      publicPath: '/_next/',
    });
    expect(logger.warn).toHaveBeenCalledWith('1 rules skipped');
    expect(logger.info).toHaveBeenCalledWith('Inlined 1 kB');
  });

  it('falls back to the base class when the port reports a fallback', async () => {
    const native = nativeReturning({
      html: null,
      logs: [],
      fallback: 'no <html>',
    });

    await expect(
      new (create(native))(NEXT_OPTIONS).process('<p>')
    ).resolves.toBe('js:<p>');
    expect(logger.warn).toHaveBeenCalledWith(
      'critters-rs fallback to JS: no <html>'
    );
  });

  it('falls back to the base class when the addon throws', async () => {
    const native: INativeCritters = {
      process: vi.fn().mockRejectedValue(new Error('panic')),
    };

    await expect(
      new (create(native))(NEXT_OPTIONS).process('<html>')
    ).resolves.toBe('js:<html>');
    expect(logger.warn).toHaveBeenCalledWith(
      'critters-rs failed, using JS: panic'
    );
  });

  it.each([
    ['pruneSource', true],
    ['compress', false],
    ['preload', 'swap'],
    ['fonts', true],
    ['reduceInlineStyles', true],
    ['inlineThreshold', 1000],
    ['keyframes', 'all'],
    ['additionalStylesheets', ['/x.css']],
    ['allowRules', ['.a']],
    ['mergeStylesheets', false],
  ])('leaves %s=%p to the base class', async (key, value) => {
    const native = nativeReturning({ html: 'rust', logs: [] });
    const Rust = create(native);

    await expect(
      new Rust({ ...NEXT_OPTIONS, [key]: value }).process('<html>')
    ).resolves.toBe('js:<html>');
    expect(native.process).not.toHaveBeenCalled();
  });

  it('leaves a custom filesystem to the base class', async () => {
    const native = nativeReturning({ html: 'rust', logs: [] });
    const critters = new (create(native))(NEXT_OPTIONS);
    (critters as unknown as { fs: unknown }).fs = {};

    await expect(critters.process('<html>')).resolves.toBe('js:<html>');
    expect(native.process).not.toHaveBeenCalled();
  });

  it('inherits static flags of the base class', () => {
    class Flagged extends FakeCritters {
      static readonly isFastCritters = true;
    }
    const Rust = createRustCritters(
      Flagged as unknown as TCrittersConstructor,
      nativeReturning({ html: '', logs: [] })
    ) as unknown as { isFastCritters?: boolean; isRustCritters?: boolean };

    expect(Rust.isFastCritters).toBe(true);
    expect(Rust.isRustCritters).toBe(true);
  });
});

// Собранный аддон (`npm run build:native`) против штатного critters.
const addonPath = process.env.CRITTERS_NATIVE_PATH || findBundledBinary() || '';
const describeAddon = existsSync(addonPath) ? describe : describe.skip;

describeAddon('critters-rs addon', () => {
  const native = loadNative(
    { CRITTERS_NATIVE_PATH: addonPath },
    logger
  ) as INativeCritters;
  const Stock = loadCritters().Critters;

  it('produces the same document as critters', async () => {
    const dist = mkdtempSync(join(tmpdir(), 'critters-rs-'));
    mkdirSync(join(dist, 'static/css'), { recursive: true });
    writeFileSync(
      join(dist, 'static/css/a.css'),
      '.used{color:red}.unused{color:blue}@media (min-width:1px){.used>p{margin:0}}' +
        '@keyframes spin{to{opacity:1}}.used{animation:spin 1s}.x:unknownfn(1){a:b}'
    );
    const html =
      '<!DOCTYPE html><html><head><link rel="stylesheet" href="/_next/static/css/a.css"/></head>' +
      '<body><div class="used"><p>x</p></div></body></html>';
    const options = { ...NEXT_OPTIONS, path: dist, logLevel: 'silent' };

    const expected = await new Stock(options).process(html);
    const Rust = createRustCritters(Stock, native);
    const result = await native.process(html, {
      path: dist,
      publicPath: '/_next/',
    });

    expect(result.html).toBe(expected);
    await expect(new Rust(options).process(html)).resolves.toBe(expected);
  });
});
