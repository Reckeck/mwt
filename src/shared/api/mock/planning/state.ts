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

/** Monthly dates (clamped to the month), weekly, every N days — from `start` up to `until`. */
export function plannedDates(schedule: Schedule, start: Date, until: Date): Date[] {
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
