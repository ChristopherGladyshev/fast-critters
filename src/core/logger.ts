import type { ILogger } from './types';

const LEVELS = ['silent', 'warn', 'info'] as const;

/**
 * Логгер по умолчанию: console с уровнем из `CRITTERS_LOG_LEVEL`
 * (`info` | `warn` | `silent`), как у Next.js для critters.
 */
export function createConsoleLogger(
  level = process.env.CRITTERS_LOG_LEVEL ??
    (process.env.NODE_ENV === 'production' ? 'warn' : 'info')
): ILogger {
  const rank = Math.max(LEVELS.indexOf(level as (typeof LEVELS)[number]), 0);
  return {
    info: (message) => rank >= 2 && console.info(message),
    warn: (message) => rank >= 1 && console.warn(message),
  };
}
