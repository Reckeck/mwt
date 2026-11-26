/**
 * Operation periods in the browser mock: the rules of `planning/period_rule.rs` and
 * `money/validate.rs` (spec op-period P1-P8, D2, S2), with the same refusals and numbers.
 * Pure functions on `YYYY-MM-DD` strings; the commands live in `commands-ledger.ts`.
 */

/** The longest `range` period, like `MAX_PERIOD_DAYS` in Rust. */
const MAX_PERIOD_DAYS = 366
/** Decimal places a flow is cut to when it is split by shares (`PART_PLACES`). */
const PART_PLACES = 2

export type MockBasis = 'period' | 'money_date'

/** The refusal every check raises, shaped like the core's `InvalidInput`. */
function invalid(message: string): never {
  throw { code: 'INVALID_INPUT', message }
}

const pad = (value: number) => String(value).padStart(2, '0')
const toDay = (stamp: Date) => `${stamp.getUTCFullYear()}-${pad(stamp.getUTCMonth() + 1)}-${pad(stamp.getUTCDate())}`
const fromDay = (raw: string) => new Date(`${raw}T00:00:00Z`)
const monthOf = (day: string) => day.slice(0, 7)

function isDay(raw: string): boolean {
  const stamp = fromDay(raw)
  return /^\d{4}-\d{2}-\d{2}$/.test(raw) && !Number.isNaN(stamp.getTime()) && toDay(stamp) === raw
}

function day(year: number, month: number, date: number): string {
  return toDay(new Date(Date.UTC(year, month, date)))
}

/** The first of the month `delta` months from the one `raw` is in. */
function firstOfMonth(raw: string, delta = 0): string {
  const stamp = fromDay(raw)
  return day(stamp.getUTCFullYear(), stamp.getUTCMonth() + delta, 1)
}

function lastOfMonth(first: string): string {
  const stamp = fromDay(first)
  return day(stamp.getUTCFullYear(), stamp.getUTCMonth() + 1, 0)
}

type Bounds = [start: string, end: string]
type Half = 'first' | 'second'

const monthBounds = (first: string): Bounds => [first, lastOfMonth(first)]

function halfBounds(first: string, half: Half): Bounds {
  const start = half === 'first' ? firstOfMonth(first) : `${monthOf(first)}-16`
  return [start, half === 'first' ? `${monthOf(first)}-15` : lastOfMonth(firstOfMonth(first))]
}

function quarterBounds(raw: string): Bounds {
  const stamp = fromDay(raw)
  const first = Math.floor(stamp.getUTCMonth() / 3) * 3
  return [day(stamp.getUTCFullYear(), first, 1), day(stamp.getUTCFullYear(), first + 3, 0)]
}

function yearBounds(raw: string): Bounds {
  const year = fromDay(raw).getUTCFullYear()
  return [day(year, 0, 1), day(year, 11, 31)]
}

/** The half-month `delta` halves away from the half `raw` is in. */
function shiftedHalfBounds(raw: string, delta: number): Bounds {
  const index = (Number(raw.slice(8, 10)) > 15 ? 1 : 0) + delta
  const half: Half = ((index % 2) + 2) % 2 === 0 ? 'first' : 'second'
  return halfBounds(firstOfMonth(raw, Math.floor(index / 2)), half)
}

/** `YYYY-MM` keys of the months `start..end` touches. */
export function monthsTouched(start: string, end: string): string[] {
  const keys: string[] = []
  if (end < start) return keys
  for (let first = firstOfMonth(start); first <= end; first = firstOfMonth(first, 1)) keys.push(monthOf(first))
  return keys
}

/** Days of `start..end` inside each month touched. */
function daysWithin(start: string, end: string): number[] {
  return monthsTouched(start, end).map((month) => {
    const from = `${month}-01` > start ? `${month}-01` : start
    const last = lastOfMonth(`${month}-01`)
    const to = last < end ? last : end
    return (fromDay(to).getTime() - fromDay(from).getTime()) / 86_400_000 + 1
  })
}

/** D2: the period a rule gives an operation of `moneyDate` (`YYYY-MM-DD`). */
export function periodOfRule(rule: PeriodRule, moneyDate: string): OpPeriod {
  if (!isDay(moneyDate)) invalid(`money date ${moneyDate}: expected YYYY-MM-DD`)
  const month = (delta: number) => firstOfMonth(moneyDate, delta)
  const of = (kind: PeriodKind, [start, end]: Bounds): OpPeriod => ({ kind, start, end })
  if (typeof rule !== 'string') {
    const { kind, offset, half } = rule.custom
    if (kind === 'half') {
      return of('half', half ? halfBounds(month(offset), half) : shiftedHalfBounds(moneyDate, offset))
    }
    if (kind === 'quarter') return of('quarter', quarterBounds(month(offset * 3)))
    if (kind === 'year') return of('year', yearBounds(month(offset * 12)))
    return of('month', monthBounds(month(offset)))
  }
  switch (rule) {
    case 'prev_month':
      return of('month', monthBounds(month(-1)))
    case 'prev_half_second':
      return of('half', halfBounds(month(-1), 'second'))
    case 'this_half_first':
      return of('half', halfBounds(moneyDate, 'first'))
    case 'this_month':
      return of('month', monthBounds(month(0)))
    case 'next_month':
      return of('month', monthBounds(month(1)))
  }
}

/** The mock's whole-number stand-in for `Amount`: eight places, enough for any asset. */
const SCALE = 1e8
const units = (raw: string | number) => Math.round(Number(raw) * SCALE)

function shown(count: number, places: number): string {
  const step = 10 ** (8 - places)
  return String(Number((Math.trunc(count / step) / 10 ** places).toFixed(places)))
}

/** Weighted shares: each cut down to `places`, the rest of the amount to the last. */
function weighted(amount: string, weights: number[], places: number): string[] {
  const total = weights.reduce((sum, weight) => sum + weight, 0)
  const whole = units(amount)
  const step = 10 ** (8 - places)
  const cut = weights.slice(0, -1).map((weight) => (total ? Math.trunc((whole * weight) / total / step) * step : 0))
  const rest = whole - cut.reduce((sum, share) => sum + share, 0)
  return [...cut, rest].map((share) => shown(share, 8))
}

const sameBounds = (left: OpPeriod | undefined, right: OpPeriod) =>
  left?.kind === right.kind && left.start === right.start && left.end === right.end

/** What the operation looked like before an edit (P8). */
export interface SpreadBefore {
  amount: string
  period?: OpPeriod
  spread?: Spread
}

/** P8: a manual spread whose amount or period moved loses its shares. */
function settleManual(spread: Spread, period: OpPeriod, amount: string, before?: SpreadBefore): Spread {
  const stale =
    before?.spread?.mode === 'manual' &&
    JSON.stringify(before.spread.shares) === JSON.stringify(spread.shares) &&
    (units(before.amount) !== units(amount) || !sameBounds(before.period, period))
  return stale ? { ...spread, shares: [] } : spread
}

/**
 * P6, P8: the spread to store. `even` and `by_days` are derived afresh, `manual` is kept (and loses
 * its shares when its amount or period moved); a one-month period keeps only what validation must refuse.
 */
export function settleSpread(
  period: OpPeriod | undefined,
  spread: Spread | undefined,
  amount: string,
  places: number,
  periodGiven: boolean,
  before?: SpreadBefore,
): Spread | undefined {
  if (!spread || !period || !isDay(period.start) || !isDay(period.end)) return spread
  const months = monthsTouched(period.start, period.end)
  if (months.length < 2) {
    const unchanged = !before?.period || sameBounds(before.period, period)
    return unchanged && (periodGiven || spread.mode === 'manual') ? spread : undefined
  }
  if (spread.mode === 'manual') return settleManual(spread, period, amount, before)
  const weights = spread.mode === 'even' ? months.map(() => 1) : daysWithin(period.start, period.end)
  const amounts = weighted(amount, weights, places)
  return { mode: spread.mode, shares: months.map((month, index) => ({ month, amount: amounts[index] })) }
}

function checkPeriodForm(period: OpPeriod): Bounds {
  if (!isDay(period.start) || !isDay(period.end)) invalid('a period date must look like YYYY-MM-DD')
  if (period.end < period.start) invalid('a period must not end before it starts')
  const { start, end, kind } = period
  const bounds = {
    month: () => start.endsWith('-01') && end === lastOfMonth(start),
    half: () =>
      (start.endsWith('-01') || start.endsWith('-16')) &&
      same(halfBounds(start, start.endsWith('-01') ? 'first' : 'second')),
    quarter: () => same(quarterBounds(start)),
    year: () => same(yearBounds(start)),
    range: () => (fromDay(end).getTime() - fromDay(start).getTime()) / 86_400_000 < MAX_PERIOD_DAYS,
  }
  function same(expected: Bounds) {
    return expected[0] === start && expected[1] === end
  }
  if (!bounds[kind]()) invalid('the period does not match its kind')
  return [start, end]
}

function checkShares(spread: Spread, months: string[], amount: string) {
  const oneEach =
    spread.shares.length === months.length && spread.shares.every((share, index) => share.month === months[index])
  if (!oneEach) invalid('a spread needs one share for each month of the period')
  if (spread.shares.some((share) => units(share.amount) < 0)) invalid('a share must not be negative')
  const sum = spread.shares.reduce((total, share) => total + units(share.amount), 0)
  if (sum !== units(amount)) invalid('the shares do not add up to the amount')
}

/** P1, P2, P5, P6: the period of an operation and its spread, refused like `check_period_meta`. */
export function checkPeriodMeta(
  spendable: boolean,
  period: OpPeriod | undefined,
  spread: Spread | undefined,
  amount: string | null,
) {
  if (!period) {
    if (spread) invalid('a spread needs a period')
    return
  }
  if (!spendable) invalid('only an income or an expense has a period')
  const [start, end] = checkPeriodForm(period)
  if (!spread) return
  const months = monthsTouched(start, end)
  if (months.length < 2) invalid('a spread needs a period over more than one month')
  if (amount === null) invalid('this operation has no amount to spread')
  checkShares(spread, months, amount)
}

/** S1, S2: the parts of an income that arrived in parts, normalised then checked like `check_parts`. */
export function settleParts(parts: ReceivedPartInput[], amount: string): ReceivedPart[] {
  const kept = parts.filter((part) => units(part.amount) !== 0)
  if (!kept.length) return []
  if (kept.length < 2) invalid('an income in parts needs at least two parts')
  for (const part of kept) {
    if (units(part.amount) <= 0) invalid('a part must be positive')
    if (part.timestamp && Number.isNaN(Date.parse(part.timestamp))) {
      invalid('the date is not a valid RFC3339 timestamp')
    }
  }
  if (kept.reduce((sum, part) => sum + units(part.amount), 0) !== units(amount)) {
    invalid('the parts must add up to the amount')
  }
  return kept.map((part) => ({ store_id: part.store_id, amount: part.amount, timestamp: part.timestamp }))
}

/** D3: the period of a new operation: the given one, else none when declined, else its category's rule. */
export function periodForNew(
  explicit: OpPeriod | undefined,
  declined: boolean,
  rule: PeriodRule | undefined,
  moneyDate: string,
): OpPeriod | undefined {
  if (explicit) return explicit
  return declined || !rule ? undefined : periodOfRule(rule, moneyDate)
}

/** The months an operation counts in and what of `value` each gets (R1-R4, P7). */
export function monthShares(
  period: OpPeriod | undefined,
  spread: Spread | undefined,
  moneyMonth: string,
  value: number,
  basis: MockBasis,
): { month: string; value: number }[] {
  if (basis === 'money_date') return [{ month: moneyMonth, value }]
  const total = spread?.shares.reduce((sum, share) => sum + units(share.amount), 0) ?? 0
  if (spread && total) {
    const step = 10 ** PART_PLACES
    let given = 0
    return spread.shares.map((share, index) => {
      const last = index === spread.shares.length - 1
      const part = last ? value - given : Math.trunc(((value * units(share.amount)) / total) * step) / step
      given += part
      return { month: share.month, value: part }
    })
  }
  const inOneMonth = period && monthOf(period.start) === monthOf(period.end)
  return [{ month: inOneMonth ? monthOf(period.start) : moneyMonth, value }]
}
