import { join } from 'node:path';

import {
  createCritters,
  loadFastCritters,
  loadNative,
} from '../src/core/loadFastCritters';

import type { INativeCritters, TCrittersConstructor } from '../src/core/types';

type TFlags = {
  isFastCritters?: boolean;
  isRustCritters?: boolean;
  isInlineAllCss?: boolean;
};

const log = { info: vi.fn(), warn: vi.fn() };
const missingAddon = join(__dirname, 'missing.node');

class FakeCritters {
  async process(html: string) {
    return html;
  }
}

const setup = (native: INativeCritters | null, isInlineAllCss = false) =>
  createCritters({
    Critters: FakeCritters as unknown as TCrittersConstructor,
    deps: { parse: () => [], compile: () => () => false },
    native,
    isInlineAllCss,
  }) as unknown as TFlags;

describe('createCritters', () => {
  const native: INativeCritters = { process: vi.fn() };

  it('builds the Rust port over FastCritters when the addon is loaded', () => {
    const Critters = setup(native);
    expect(Critters.isRustCritters).toBe(true);
    expect(Critters.isFastCritters).toBe(true);
    expect(Critters.isInlineAllCss).toBeUndefined();
  });

  it('keeps FastCritters without the addon', () => {
    const Critters = setup(null);
    expect(Critters.isFastCritters).toBe(true);
    expect(Critters.isRustCritters).toBeUndefined();
  });

  it('inlines all CSS on request', () => {
    expect(setup(native, true).isInlineAllCss).toBe(true);
  });
});

describe('loadNative', () => {
  beforeEach(() => log.warn.mockClear());

  it('skips the addon without a path or with CRITTERS_NATIVE=false', () => {
    expect(loadNative({}, log, null)).toBeNull();
    expect(
      loadNative(
        { CRITTERS_NATIVE_PATH: missingAddon, CRITTERS_NATIVE: 'false' },
        log,
        missingAddon
      )
    ).toBeNull();
    expect(log.warn).not.toHaveBeenCalled();
  });

  it('logs a load error and keeps JS', () => {
    expect(loadNative({ CRITTERS_NATIVE_PATH: missingAddon }, log)).toBeNull();
    expect(loadNative({}, log, missingAddon)).toBeNull();
    expect(log.warn).toHaveBeenCalledWith(
      expect.stringContaining(
        `critters-rs addon failed to load from ${missingAddon}`
      )
    );
  });
});

describe('loadFastCritters', () => {
  beforeEach(() => log.info.mockClear());

  it('logs which implementation runs', () => {
    const Critters = loadFastCritters(
      { CRITTERS_NATIVE: 'false' },
      log
    ) as unknown as TFlags;
    expect(Critters.isFastCritters).toBe(true);
    expect(log.info).toHaveBeenCalledWith('critters: js');
  });

  it('returns stock critters with FAST_CRITTERS=false', () => {
    const Critters = loadFastCritters(
      { FAST_CRITTERS: 'false' },
      log
    ) as unknown as TFlags;
    expect(Critters.isFastCritters).toBeUndefined();
    expect(log.info).toHaveBeenCalledWith('critters: stock');
  });

  it('inlines all CSS with INLINE_ALL_CSS=true', () => {
    const Critters = loadFastCritters(
      { INLINE_ALL_CSS: 'true' },
      log
    ) as unknown as TFlags;
    expect(Critters.isInlineAllCss).toBe(true);
  });
});
