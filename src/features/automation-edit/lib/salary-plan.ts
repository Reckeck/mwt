import { fiatRef } from '@/shared/lib/asset-ref'
import { dateInputToIso, localDate } from '@/shared/lib/planning'

/** `For which month` of a payout: the month before the one it comes in, or the same one. */
export type SalaryForMonth = 'previous' | 'current'

/** The months in the order the select lists them; the value is `for_month_offset` of the automation. */
export const SALARY_FOR_MONTH_OFFSET: Record<SalaryForMonth, number> = { previous: -1, current: 0 }

/** One payout of the month: a salary, an advance — its own name, day, month it is for and amount. */
export interface SalaryPayout {
  name: string
  day: number
  forMonth: SalaryForMonth
  shift: WorkdayShift
  /** `Varies — ask`: the amount is asked at each confirmation, `amount` is the usual one. */
  varies: boolean
  amount: string
  currency: string
}

/** What `Salary` (Auto-2 · 9) asks beyond the payouts: where it comes from and to. */
export interface SalaryDraft extends SalaryPayout {
  category: string
  storeId: string
}

/** The two payouts a month can hold: the salary on the 5th for the month before, the advance on the 20th for this one. */
export function defaultPayout(kind: 'salary' | 'advance', name: string, currency: string): SalaryPayout {
  return {
    name,
    day: kind === 'salary' ? 5 : 20,
    forMonth: kind === 'salary' ? 'previous' : 'current',
    shift: 'back',
    varies: true,
    amount: '',
    currency,
  }
}

/** Part of the salary `Pay yourself first` moves to cash after it comes. */
export const SALARY_SHARE_PCT = 10

/** The monthly top-up the template books: the salary's category, wallet and usual amount. */
export function salaryInput(draft: SalaryDraft): AutomationInput {
  const asset = fiatRef(draft.currency)
  return {
    name: draft.name.trim(),
    active: true,
    preset: 'salary',
    schedule: { monthly: { day: draft.day } },
    starts_at: dateInputToIso(localDate(new Date())),
    template: {
      type: 'topup',
      common: {
        timestamp: '',
        comment: '',
        categories: [draft.category],
        flow_id: null,
        contact_id: null,
        price_usd: null,
        fx_override: null,
        price: null,
        price_currency: null,
      },
      store_id: draft.storeId,
      asset: { symbol: asset.symbol, name: asset.name, coingecko_id: null, chain: null, contract: null },
      amount: draft.amount.trim(),
      fee: null,
      from: null,
      details: { cash_breakdown: null, unit_price: null, network: null, tx_hash: null },
    },
    amount_mode: draft.varies ? 'varies' : 'fixed',
    workday_shift: draft.shift,
    time: null,
    asks: false,
    for_month_offset: SALARY_FOR_MONTH_OFFSET[draft.forMonth],
  }
}
