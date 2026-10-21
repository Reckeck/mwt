/** The capital checkup (plan C4): a simplified copy of `service/health.rs`. */

import { isPassive } from '@/shared/lib/categories'
import { txLegs, txMeta } from '@/shared/lib/transactions'
import { money, monthTotals } from './budget-calc'
import { emergencyStatus, storeValue } from './emergency'
import { fail, iso, now } from './state'
import type { PlanningMockContext } from './state'

const DTI_HIGH = 50
const DTI_ELEVATED = 30
const RECENT_MONTHS = 3
const YEAR = 12
/** Stands in for the key rate the core would read. */
const KEY_RATE = 16

/** `YYYY-MM` of the `count` finished months, oldest first. */
function finishedMonths(count: number): string[] {
  const today = now()
  return Array.from({ length: count }, (_unused, index) => {
    const date = new Date(today.getFullYear(), today.getMonth() - count + index, 1)
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`
  })
}

function loanOf(store: Store, ctx: PlanningMockContext): LoanStatus {
  const terms = store.liability
  const rate = terms?.rate ?? null
  const owed = Math.max(0, -storeValue(store.id, ctx))
  const kind = terms?.kind ?? 'other'
  let loanClass: LoanClass = 'unknown'
  if (kind === 'credit_card' && owed > 0) loanClass = 'costly'
  else if (rate !== null) loanClass = Number(rate) > KEY_RATE ? 'costly' : Number(rate) < 8 ? 'cheap' : 'normal'
  return {
    store_id: store.id,
    kind,
    own_currency: 'usd',
    owed: money(owed),
    rate,
    benchmark: rate === null ? null : String(KEY_RATE),
    inflation: '8',
    class: loanClass,
    payment: terms?.monthly_payment ?? '0',
    payment_from_fact: false,
  }
}

function passiveIncome(ctx: PlanningMockContext, eMand: number | null, invested: number, rate: number | null) {
  const passive = new Set(ctx.categories.filter(isPassive).map((category) => category.key))
  const byMonth = new Map<string, number>()
  for (const tx of ctx.transactions) {
    if (!('topup' in tx) || !txMeta(tx).categories.some((key) => passive.has(key))) continue
    const value = txLegs(tx)
      .filter((leg) => leg.role === 'principal')
      .reduce((sum, leg) => sum + Number(leg.amount) * ctx.priceOf(leg.asset.symbol), 0)
    const month = txMeta(tx).timestamp.slice(0, 7)
    byMonth.set(month, (byMonth.get(month) ?? 0) + value)
  }
  const months = finishedMonths(YEAR).map((month) => ({ month, amount: money(byMonth.get(month) ?? 0) }))
  const total = months.reduce((sum, row) => sum + Number(row.amount), 0)
  const share = (monthly: number) => (eMand ? money((monthly / eMand) * 100) : null)
  return {
    currency: ctx.budgetCurrency,
    months,
    total_12m: money(total),
    reinvested_12m: '0',
    by_category: [],
    this_month: money(byMonth.get(iso(now()).slice(0, 7)) ?? 0),
    e_mand: eMand === null ? null : money(eMand),
    freedom_fact_pct: share(total / YEAR),
    invested: money(invested),
    expected_pct: String(rate ?? KEY_RATE),
    freedom_potential_pct: share((invested * (rate ?? KEY_RATE)) / 100 / YEAR),
  } satisfies PassiveIncome
}

function reasonsOf(emergency: EmergencyStatus, debts: DebtLoad, savingsAvg: number | null, target: number) {
  const reasons: HealthReason[] = []
  const add = (code: HealthReasonCode, level: HealthColor, extra: Partial<HealthReason> = {}) =>
    reasons.push({ code, level, amount: null, pct: null, subject: null, ...extra })
  for (const loan of debts.loans) {
    if (loan.class === 'costly')
      add('costly_loan', 'red', { subject: loan.store_id, amount: loan.owed, pct: loan.rate })
    if (loan.class === 'unknown') add('loan_without_rate', 'green', { subject: loan.store_id })
  }
  if (emergency.e_mand === null && debts.income === null) {
    add('no_history', 'no_data')
    return reasons
  }
  const dti = debts.dti_pct === null ? null : Number(debts.dti_pct)
  if (dti !== null && dti > DTI_HIGH) add('dti_high', 'red', { pct: debts.dti_pct })
  else if (dti !== null && dti > DTI_ELEVATED) add('dti_elevated', 'yellow', { pct: debts.dti_pct })
  const covered = emergency.months_covered === null ? null : Number(emergency.months_covered)
  const eMand = Number(emergency.e_mand ?? 0)
  if (!emergency.fund.scope.store_ids.length) add('fund_not_set', 'yellow')
  else if (covered !== null && covered < 1) {
    add('fund_below_one', 'red', { pct: emergency.months_covered, amount: money(eMand - Number(emergency.total)) })
  } else if (covered !== null && covered < 3) {
    add('fund_below_three', 'yellow', {
      pct: emergency.months_covered,
      amount: money(eMand * 3 - Number(emergency.total)),
    })
  }
  if (savingsAvg !== null && savingsAvg < target) add('savings_below_plan', 'yellow', { pct: money(savingsAvg) })
  return reasons
}

const RANK: Record<HealthColor, number> = { no_data: 0, green: 1, yellow: 2, red: 3 }

function capitalHealth(ctx: PlanningMockContext, settings: Settings): HealthReport {
  const emergency = emergencyStatus(ctx)
  const savings: SavingsMonth[] = finishedMonths(YEAR).map((month) => {
    const totals = monthTotals(ctx, month)
    const saved = totals.income - totals.fixed - totals.daily
    return {
      month,
      income: money(totals.income),
      saved: money(saved),
      saved_pct: totals.income ? money((saved / totals.income) * 100) : null,
    }
  })
  const recent = savings.filter((row) => Number(row.income) > 0).slice(-RECENT_MONTHS)
  const income = recent.reduce((sum, row) => sum + Number(row.income), 0)
  const savingsAvg = income ? (recent.reduce((sum, row) => sum + Number(row.saved), 0) / income) * 100 : null
  const loans = ctx.stores
    .filter((store) => store.kind === 'liability' && !store.archived)
    .map((store) => loanOf(store, ctx))
  const payments = loans.reduce((sum, loan) => sum + Number(loan.payment), 0)
  const avgIncome = recent.length ? income / recent.length : null
  const debts: DebtLoad = {
    currency: ctx.budgetCurrency,
    loans,
    owed: money(loans.reduce((sum, loan) => sum + Number(loan.owed), 0)),
    payments: money(payments),
    income: avgIncome === null ? null : money(avgIncome),
    income_months: recent.length,
    dti_pct: avgIncome ? money((payments / avgIncome) * 100) : null,
    key_rate: String(KEY_RATE),
  }
  const invested = ctx.stores
    .filter((store) => store.purpose === 'investment' && store.kind !== 'liability' && !store.archived)
    .reduce((sum, store) => sum + Math.max(0, storeValue(store.id, ctx)), 0)
  const eMand = emergency.e_mand === null ? null : Number(emergency.e_mand)
  const rate = settings.freedom_rate === null ? null : Number(settings.freedom_rate)
  const target = Number(settings.savings_rate) * 100
  const reasons = reasonsOf(emergency, debts, savingsAvg, target).sort(
    (left, right) => RANK[right.level] - RANK[left.level],
  )
  const worst = reasons[0]?.level ?? 'green'
  const noHistory = reasons.some((reason) => reason.code === 'no_history')
  const color: HealthColor =
    RANK[worst] <= RANK.green && noHistory ? 'no_data' : RANK[worst] < RANK.green ? 'green' : worst
  return {
    color,
    reasons,
    currency: ctx.budgetCurrency,
    emergency,
    debts,
    passive: passiveIncome(ctx, eMand, invested, rate),
    savings,
    savings_target_pct: money(target),
    savings_avg_pct: savingsAvg === null ? null : money(savingsAvg),
    creep: null,
    freedom_rate: settings.freedom_rate,
  }
}

export function planHealth(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
  settings: Settings,
): unknown {
  switch (cmd) {
    case 'capital_health':
      return capitalHealth(ctx, settings)
    case 'health_settings_set': {
      const rate = (args?.expectedReturn as string | null | undefined) ?? null
      if (rate !== null && Number(rate) < 0) fail('the expected return cannot be negative')
      settings.freedom_rate = rate
      return null
    }
    default:
      return undefined
  }
}
