/** Shared wording and keys for planning: schedules, rejections, query keys. */

type Translate = (key: string, params?: Record<string, unknown>) => string

/** Query keys a planning write makes stale. */
export const PLANNING_KEYS = [
  ['automations'],
  ['automation-runs'],
  ['budgets'],
  ['budget-status'],
  ['daily-budget'],
  ['month-summary'],
  ['goals'],
  ['goal-progress'],
  ['goal-calendar'],
  ['reminders'],
  ['price-alerts'],
  ['upcoming'],
]

/** Keys a transaction write makes stale on top of the planning ones. */
export const LEDGER_KEYS = [
  ['transactions'],
  ['store'],
  ['store-history'],
  ['store-pnl'],
  ['summary'],
  ['stores'],
  ['history'],
  ['net-worth'],
]

export const WEEKDAYS = [0, 1, 2, 3, 4, 5, 6] as const

/** «Ежемесячно, 15-го» / «Каждый понедельник» / «Каждые 3 дня». */
export function scheduleText(schedule: Schedule, t: Translate): string {
  if ('monthly' in schedule) return t('schedule.monthlyOn', { day: schedule.monthly.day })
  if ('weekly' in schedule) return t('schedule.weeklyOn', { weekday: t(`schedule.weekday.${schedule.weekly.weekday}`) })
  return t('schedule.everyNDays', { days: schedule.every_n_days.days })
}

/** Why a run created nothing, in words. `storeName` resolves store ids. */
export function rejectText(reason: RejectReason, t: Translate, storeName: (id: string) => string): string {
  if ('insufficient_funds' in reason) {
    const data = reason.insufficient_funds
    return t('automation.rejected.funds', {
      store: storeName(data.store_id),
      available: data.available,
      required: data.required,
      asset: data.asset.symbol,
    })
  }
  if ('credit_limit_exceeded' in reason) {
    const data = reason.credit_limit_exceeded
    return t('automation.rejected.limit', {
      store: storeName(data.store_id),
      limit: data.limit,
      required: data.required,
    })
  }
  return reason.invalid.message
}

/** `YYYY-MM-DD` of a local `Date`. */
export function localDate(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`
}

/** A `date` input value (local midnight) as RFC3339 UTC. */
export function dateInputToIso(value: string): string {
  const parsed = new Date(`${value}T00:00`)
  return Number.isNaN(parsed.getTime()) ? '' : parsed.toISOString()
}

/** RFC3339 as a `date` input value in local time. */
export function isoToDateInput(iso: string | null | undefined): string {
  if (!iso) return ''
  const parsed = new Date(iso)
  return Number.isNaN(parsed.getTime()) ? '' : localDate(parsed)
}
