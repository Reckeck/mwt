/**
 * Numeric limits of the UI, one home. Each number matches the Rust (or inventory) counterpart named in its
 * comment; a limit is shown as a field message on save, never as a silent clamp.
 */

/** Most integer digits of an amount; Rust `Amount` input bound 1e18 (`crates/base/src/amount.rs`). */
export const MAX_INTEGER_DIGITS = 18

/** Earliest year a date field accepts; the oldest stored moment the core handles (`crates/money/src/validate.rs`). */
export const MIN_DATE_YEAR = 1970
/** Latest year a date field accepts; keeps per-day loops and chrono arithmetic far from `chrono::MAX` (`crates/reports/src/calendar.rs`). */
export const MAX_DATE_YEAR = 2100

/** Longest `range` operation period in days; Rust `MAX_PERIOD_DAYS` (`crates/base/src/limits.rs`). */
export const MAX_PERIOD_DAYS = 366
/** Most months of a report; Rust `MAX_REPORT_MONTHS` (`crates/reports`). */
export const MAX_REPORT_MONTHS = 120
/** Most months `get_store_months` returns; Rust clamp 1..36 (`crates/money/src/store_months.rs`). */
export const MAX_STORE_MONTHS = 36
/** Most repeats listed for a reminder; Rust `MAX_REPEATS_LISTED` (`crates/planning`). */
export const MAX_REPEATS_LISTED = 60

/** Most items of a workflow list; Rust workflow `MAX_ITEMS` (`crates/usecases`). */
export const MAX_ROWS_ITEMS = 200
/** Most steps of a workflow; Rust workflow `MAX_STEPS` (`crates/usecases`). */
export const MAX_ROWS_STEPS = 500

/** Longest invest horizon in years (frontend `MAX_YEARS`; core `MAX_TERM_MONTHS` = 1200 in `calc/invest.rs`). */
export const MAX_YEARS = 100
/** Largest invest rate in percent (frontend `MAX_RATE`; core `rate_pct <= 1000`, `calc/invest.rs`). */
export const MAX_RATE_PERCENT = 200
/** Largest rate the core accepts, percent (`calc/invest.rs`). */
export const MAX_CORE_RATE_PERCENT = 1000
/** Longest invest term in months; Rust `MAX_TERM_MONTHS` (`calc/invest.rs`). */
export const MAX_TERM_MONTHS = 1200
/** Largest share, percent (goal return, asset class, revalue). */
export const MAX_PERCENT = 100
/** Longest emergency fund in months; Rust `MAX_FUND_MONTHS` (`crates/planning`). */
export const FUND_MONTHS_MAX = 24
/** Shortest auto-lock in minutes; Rust `AUTOLOCK_MIN_MINUTES` (`crates/usecases/src/appconfig.rs`). */
export const AUTOLOCK_MIN = 5
/** Longest auto-lock in minutes; Rust `AUTOLOCK_MAX_MINUTES` (`crates/usecases/src/appconfig.rs`). */
export const AUTOLOCK_MAX = 60
/** Largest odometer-style distance, km (frontend `KM_MAX`, 5 million). */
export const KM_MAX = 5_000_000
/** Every-N-days repeat bounds (frontend, 1..3650). */
export const EVERY_DAYS_MIN = 1
export const EVERY_DAYS_MAX = 3650
/** Day-of-month bounds. */
export const DAY_MIN = 1
export const DAY_MAX = 31
/** Deposit term in months (frontend, 1..120). */
export const DEPOSIT_TERM_MIN = 1
export const DEPOSIT_TERM_MAX = 120

/** True when a `YYYY`, `YYYY-MM`, `YYYY-MM-DD` or `YYYY-MM-DDTHH:mm` value lies inside the date year range; blank passes. */
export function isDateWithinBounds(value: string | null | undefined): boolean {
  const match = /^(\d{4,})/.exec(value ?? '')
  if (!match) return true
  const year = Number(match[1])
  return year >= MIN_DATE_YEAR && year <= MAX_DATE_YEAR
}

/** First and last allowed day as `YYYY-MM-DD`. */
export const DATE_MIN = `${MIN_DATE_YEAR}-01-01`
export const DATE_MAX = `${MAX_DATE_YEAR}-12-31`

/** A picker's own `min`/`max`, never looser than the global date range. */
export function clampToDateBounds(own: string | undefined, edge: 'min' | 'max'): string {
  const global = edge === 'min' ? DATE_MIN : DATE_MAX
  if (!own) return global
  return (edge === 'min' ? own < global : own > global) ? global : own
}
