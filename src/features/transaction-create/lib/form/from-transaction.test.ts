import { describe, expect, it } from 'vitest'

import { formFromTransaction } from './from-transaction'
import { formToInput } from './to-input'

const usd = { symbol: 'USD', name: 'USD', coingecko_id: null, chain: null, contract: null }

function topup(forMonth?: string): Transaction {
  const record = {
    meta: {
      id: 't1',
      timestamp: '2026-10-05T10:00:00Z',
      comment: 'salary',
      categories: [],
      flow_id: null,
      contact_id: null,
      origin: { kind: 'manual' },
    },
    store_id: 's1',
    asset: usd,
    amount: '1000',
    fee: null,
    from: null,
    details: { cash_breakdown: null, unit_price: null, network: null, tx_hash: null, for_month: forMonth },
    price: {},
  }
  return { topup: record } as unknown as Transaction
}

describe('for_month on edit', () => {
  it('is loaded into the form and sent back unchanged', () => {
    const form = formFromTransaction(topup('2026-09'), 's1')
    expect(form.forMonth).toBe('2026-09')
    const input = formToInput(form)
    expect(input && 'details' in input && input.details.for_month).toBe('2026-09')
  })

  it('is absent when the stored record has none', () => {
    const form = formFromTransaction(topup(), 's1')
    expect(form.forMonth).toBe('')
    const input = formToInput(form)
    expect(input && 'details' in input && (input.details.for_month ?? null)).toBeNull()
  })
})
