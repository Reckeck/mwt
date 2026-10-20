/** Planning mock state: the lists, the last context, small helpers. */

/** What the planning mock needs from the ledger mock. */
export interface PlanningMockContext {
  transactions: Transaction[]
  categories: Category[]
  stores: Store[]
  priceOf: (symbol: string) => number
  transactionFrom: (input: TransactionInput, ts: string) => Transaction
  budgetCurrency: string
}

export const automations: Automation[] = []
export const runs: AutomationRun[] = []
export const budgets: Budget[] = []
export const goals: Goal[] = []
export const reminders: Reminder[] = []
export const alerts: PriceAlert[] = []
export const utilities: UtilityItem[] = []
export const readings: MeterReading[] = []

/** The context of the last call; a holder, since an imported binding cannot be assigned. */
export const mockContext: { value: PlanningMockContext | null } = { value: null }

export const uuid = () => crypto.randomUUID()
export const isRejected = (outcome: AutomationOutcome) => typeof outcome === 'object' && 'rejected' in outcome
export const now = () => new Date()
export const iso = (date: Date) => date.toISOString()
export const fail = (message: string, code = 'INVALID_INPUT') => {
  throw { code, message }
}

export function draftOf(tx: Transaction): TransactionDraft {
  if ('topup' in tx || 'payment' in tx || 'transfer' in tx || 'convert' in tx) return tx as TransactionDraft
  return fail('an automation repeats a top-up, payment, transfer or exchange')
}

/** Public holidays of Russia by `month-day`, as the Rust calendar has them. */
const RU_HOLIDAYS = new Set([
  '1-1',
  '1-2',
  '1-3',
  '1-4',
  '1-5',
  '1-6',
  '1-7',
  '1-8',
  '2-23',
  '3-8',
  '5-1',
  '5-9',
  '6-12',
  '11-4',
])

/** A date on a Saturday, a Sunday or a holiday moved to a working day before or after. */
export function moveOffDayOff(date: Date, shift: WorkdayShift): Date {
  if (shift === 'none') return date
  const moved = new Date(date)
  const step = shift === 'back' ? -1 : 1
  for (let tries = 0; tries < 60; tries += 1) {
    const day = moved.getDay()
    if (day !== 0 && day !== 6 && !RU_HOLIDAYS.has(`${moved.getMonth() + 1}-${moved.getDate()}`)) return moved
    moved.setDate(moved.getDate() + step)
  }
  return date
}

/**
 * Monthly dates (clamped to the month), weekly, every N days — from `start`
 * up to `until`, moved off days off as `shift` says (looked for 20 days past
 * `until`, so a date moved back into the window still counts).
 */
export function plannedDates(schedule: Schedule, start: Date, until: Date, shift: WorkdayShift = 'none'): Date[] {
  if (shift !== 'none') {
    const later = new Date(until.getTime() + 20 * 86_400_000)
    const seen = new Set<number>()
    return plannedDates(schedule, start, later)
      .map((date) => moveOffDayOff(date, shift))
      .filter((date) => date <= until && !seen.has(date.getTime()) && Boolean(seen.add(date.getTime())))
  }
  const out: Date[] = []
  const at = (year: number, month: number, day: number) => new Date(year, month, day, 9)
  if ('monthly' in schedule) {
    let cursor = new Date(start.getFullYear(), start.getMonth(), 1)
    while (cursor <= until) {
      const last = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 0).getDate()
      const date = at(cursor.getFullYear(), cursor.getMonth(), Math.min(schedule.monthly.day, last))
      if (date >= start && date <= until) out.push(date)
      cursor = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 1)
    }
  } else {
    const step = 'weekly' in schedule ? 7 : Math.max(1, schedule.every_n_days.days)
    const cursor = at(start.getFullYear(), start.getMonth(), start.getDate())
    if ('weekly' in schedule) {
      const weekday = (cursor.getDay() + 6) % 7
      cursor.setDate(cursor.getDate() + ((schedule.weekly.weekday - weekday + 7) % 7))
    }
    while (cursor <= until) {
      if (cursor >= start) out.push(new Date(cursor))
      cursor.setDate(cursor.getDate() + step)
    }
  }
  return out
}
export function replace<T extends { id: string }>(list: T[], id: string, next: Omit<T, 'id'>): T {
  const index = list.findIndex((entry) => entry.id === id)
  if (index < 0) fail('not found', 'MISSING')
  list[index] = { ...list[index], ...next, id }
  return list[index]
}

export function remove<T extends { id: string }>(list: T[], id: string) {
  const index = list.findIndex((entry) => entry.id === id)
  if (index < 0) fail('not found', 'MISSING')
  list.splice(index, 1)
}
