import { fiatRef } from '@/shared/lib/asset-ref'
import { dateInputToIso, localDate } from '@/shared/lib/planning'

/** What «Зарплата» (Auto-2 · 9) asks: where it comes from and to, how much, on which day. */
export interface SalaryDraft {
  category: string
  storeId: string
  /** «Меняется — спрашивать»: the amount is asked at each confirmation, `amount` is the usual one. */
  varies: boolean
  amount: string
  currency: string
  day: number
  shift: WorkdayShift
}

/** Part of the salary «Заплати себе первым» moves to cash after it comes. */
export const SALARY_SHARE_PCT = 10

/** The monthly top-up the template books: the salary's category, wallet and usual amount. */
export function salaryInput(draft: SalaryDraft, name: string): AutomationInput {
  const asset = fiatRef(draft.currency)
  return {
    name,
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
  }
}
