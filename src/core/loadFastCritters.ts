import { createRequire } from 'node:module';

import { createConsoleLogger } from './logger';

import { createFastCritters } from './createFastCritters';
import { createRustCritters } from './createRustCritters';
import { createInlineAllCss } from './inlineAllCss';
import { loadCritters } from './loadCritters';
import { findBundledBinary } from './native';

import type {
  ICrittersSetup,
  INativeCritters,
  TCrittersConstructor,
  TCrittersEnv,
  TCrittersLog,
} from './types';

/**
 * Аддон из `CRITTERS_NATIVE_PATH`, иначе собранный в `bin/` пакета;
 * при `CRITTERS_NATIVE=false` остаётся JS.
 */
export function loadNative(
  env: TCrittersEnv,
  log: TCrittersLog,
  bundled: string | null = findBundledBinary()
): INativeCritters | null {
  const path = env.CRITTERS_NATIVE_PATH || bundled;
  if (!path || env.CRITTERS_NATIVE === 'false') {
    return null;
  }
  try {
    return createRequire(__filename)(path) as INativeCritters;
  } catch (error) {
    log.warn(
      `critters-rs addon failed to load from ${path}: ${(error as Error).message}`
    );
    return null;
  }
}

/** Rust-порт с FastCritters как запасным вариантом, при `isInlineAllCss` — весь CSS страницы. */
export function createCritters({
  Critters,
  deps,
  native,
  isInlineAllCss,
}: ICrittersSetup): TCrittersConstructor {
  const fast = createFastCritters(Critters, deps);
  const critical = native ? createRustCritters(fast, native) : fast;
  return isInlineAllCss ? createInlineAllCss(critical) : critical;
}

/** Класс, который Next.js получает по `require('critters')`; `FAST_CRITTERS=false` — штатный critters. */
export function loadFastCritters(
  env: TCrittersEnv = process.env,
  log: TCrittersLog = createConsoleLogger()
): TCrittersConstructor {
  const loaded = loadCritters();
  if (env.FAST_CRITTERS === 'false') {
    log.info('critters: stock');
    return loaded.Critters;
  }

  const native = loadNative(env, log);
  log.info(`critters: ${native ? 'rust' : 'js'}`);
  return createCritters({
    ...loaded,
    native,
    isInlineAllCss: env.INLINE_ALL_CSS === 'true',
  });
}
