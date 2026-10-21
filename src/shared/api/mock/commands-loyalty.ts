/** Commands: loyalty points, cashback, deposits and interest. */

import { mockTransactionFrom } from './ledger'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockPrices, mockPush, mockStores, priceOf } from './demo'
import { mockOwnPositions } from './tree'
import { storeTreeCommands } from './commands-store-tree'

/** Commands: loyalty points, cashback, deposits and interest. */
export const loyaltyDepositCommands: MockCommands = {
  set_loyalty_info: ({ body }) => {
    const input = body as SetLoyaltyInfoInput
    const store = mockStores.find((entry) => entry.id === input.store_id)
    if (!store) return Promise.reject({ code: 'MISSING', message: 'store' })
    store.loyalty = { ...input.info, points_symbol: input.info.points_symbol.toUpperCase() }
    mockPrices[store.loyalty.points_symbol] = Number(input.info.point_value) * (priceOf(input.info.currency) || 1)
    return resolve(store)
  },
  create_loyalty_program: async ({ body }) => {
    const input = body as CreateLoyaltyProgramInput
    const symbol = input.info.points_symbol.trim().toUpperCase()
    if (mockStores.some((store) => store.loyalty?.points_symbol === symbol))
      return Promise.reject({ code: 'INVALID_INPUT', message: `${symbol} already names another programme's points` })
    const store = (await storeTreeCommands.create_store({
      cmd: 'create_store',
      args: {},
      body: { name: input.name, kind: 'loyalty', color: input.color, note: '', contact_id: input.contact_id },
    })) as Store
    return loyaltyDepositCommands.set_loyalty_info({
      cmd: 'set_loyalty_info',
      args: {},
      body: { store_id: store.id, info: input.info },
    })
  },
  adjust_points: ({ body }) => {
    const input = body as AdjustPointsInput
    const store = mockStores.find((entry) => entry.id === input.store_id)
    const symbol = store?.loyalty?.points_symbol
    if (!store || !symbol) return Promise.reject({ code: 'INVALID_INPUT', message: 'set up the programme first' })
    const positive = Number(input.delta) > 0
    const tx = mockTransactionFrom(
      {
        type: positive ? 'topup' : 'payment',
        common: {
          timestamp: input.timestamp ?? new Date().toISOString(),
          comment: input.comment,
          categories: positive ? ['cashback'] : [],
          flow_id: null,
          contact_id: null,
          price_usd: null,
          fx_override: null,
          price: null,
          price_currency: null,
        },
        store_id: store.id,
        asset: { symbol, name: store.name, coingecko_id: null, chain: null, contract: null },
        amount: String(Math.abs(Number(input.delta))),
        fee: null,
        ...(positive ? { from: null } : { to: null }),
        details: {
          cash_breakdown: null,
          unit_price: null,
          network: null,
          tx_hash: null,
          source_store_id: input.source_store_id,
        },
      } as TransactionInput,
      priceOf,
    )
    return resolve(mockPush(tx))
  },
  get_cashback_report: (_call) => {
    const programmes: LoyaltyBalance[] = mockStores
      .filter((store) => store.kind === 'loyalty' && store.loyalty)
      .map((store) => {
        const points = mockOwnPositions(store.id).reduce((sum, position) => sum + Number(position.quantity), 0)
        return {
          store_id: store.id,
          name: store.name,
          points_symbol: store.loyalty?.points_symbol ?? '',
          points: String(points),
          value_base: String(points * priceOf(store.loyalty?.points_symbol ?? '')),
          expires_at: store.loyalty?.expires_at ?? null,
          expiring_points: store.loyalty?.expiring_points ?? null,
        }
      })
    const card = mockStores.find((store) => store.kind === 'bank')
    return resolve({
      base_currency: 'usd',
      cashback_base: '12.4',
      points_base: programmes.reduce((sum, row) => sum + Number(row.value_base), 0).toFixed(2),
      by_source: card ? [{ store_id: card.id, name: card.name, cashback_base: '12.4', points_base: '0' }] : [],
      programmes,
    } satisfies CashbackReport)
  },
  set_deposit_terms: ({ body }) => {
    const input = body as SetDepositTermsInput
    const store = mockStores.find((entry) => entry.id === input.store_id)
    if (!store) return Promise.reject({ code: 'MISSING', message: 'store' })
    store.deposit = input.terms ?? undefined
    return resolve(store)
  },
  get_deposit_report: ({ args }) => {
    const store = mockStores.find((entry) => entry.id === args?.storeId)
    const terms = store?.deposit
    const rate = terms?.rates.at(-1)?.rate ?? null
    const balance = mockOwnPositions(String(args?.storeId)).reduce(
      (sum, position) => sum + Number(position.quantity),
      0,
    )
    const yearly = rate ? (balance * Number(rate)) / 100 : 0
    const today = new Date()
    return resolve({
      store_id: String(args?.storeId),
      currency: 'rub',
      balance: String(balance),
      rate,
      rate_since: terms?.rates.at(-1)?.from ?? null,
      accrued_month: ((yearly / 365) * today.getDate()).toFixed(2),
      accrued_year: ((yearly / 365) * (today.getMonth() * 30 + today.getDate())).toFixed(2),
      posted_year: '0',
      projected_year: yearly.toFixed(2),
      matures_at: null,
      at_maturity: null,
      pds: terms?.pds
        ? {
            year: today.getFullYear(),
            contributions: String(balance),
            cofinancing: String(Math.min(36000, balance)),
            to_full_cofinancing: String(Math.max(0, 36000 - balance)),
            ratio: 1,
            cofinancing_due: `${today.getFullYear() + 1}-08-31`,
            ends_at: `${Number(terms.pds.first_contribution_at.slice(0, 4)) + 10}${terms.pds.first_contribution_at.slice(4)}`,
            deduction: String(Math.min(balance, 400000) * 0.13),
          }
        : null,
    } satisfies DepositReport)
  },
  get_interest_tax: (_call) => {
    return resolve({
      year: new Date().getFullYear(),
      max_key_rate: '21',
      limit: '210000',
      income: '48250.5',
      taxable: '0',
      tax: '0',
      remaining_free: '161749.5',
    } satisfies InterestTaxReport)
  },
}
