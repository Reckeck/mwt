import { describe, expect, it } from 'vitest'

import {
  templateAssetOf,
  templateDraftOf,
  templateErrors,
  templateKindOf,
  templateRetyped,
  templateToInput,
} from './template-input'

const draft = { kind: 'payment' as const, amount: '12,5', category: 'food', comment: ' lunch ', to: '' }

describe('template input', () => {
  it('builds a payment with a dot decimal and a trimmed note', () => {
    const input = templateToInput(draft, 'wallet', null, 'rub')
    expect(input).toMatchObject({ type: 'payment', store_id: 'wallet', amount: '12.5' })
    expect(input.common.categories).toEqual(['food'])
    expect(input.common.comment).toBe('lunch')
  })

  it('builds a transfer between two wallets without a category', () => {
    const input = templateToInput({ ...draft, kind: 'transfer', to: 'other' }, 'wallet', null, 'rub')
    expect(input).toMatchObject({ type: 'transfer', from: { store: 'wallet' }, to: { store: 'other' } })
    expect(input.common.categories).toEqual([])
  })

  it('asks for an amount, a wallet and a category', () => {
    expect(Object.keys(templateErrors({ ...draft, amount: '', category: '' }, ''))).toEqual([
      'amount',
      'store',
      'category',
    ])
    expect(templateErrors(draft, 'wallet')).toEqual({})
  })

  it('asks for the receiving wallet of a transfer only', () => {
    expect(Object.keys(templateErrors({ ...draft, kind: 'transfer', category: '' }, 'wallet'))).toEqual(['to'])
  })

  it('reads a stored template and refuses an exchange', () => {
    const stored = { payment: { amount: '3', meta: { categories: ['fun'], comment: 'x' } } } as unknown as Transaction
    expect(templateKindOf(stored)).toBe('payment')
    expect(templateDraftOf(stored)).toMatchObject({ kind: 'payment', amount: '3', category: 'fun', comment: 'x' })
    expect(templateKindOf({ convert: {} } as unknown as Transaction)).toBeNull()
  })
})

describe('template start', () => {
  it('maps a wizard draft and falls back to an expense for an exchange', async () => {
    const { templateStartOf } = await import('./template-input')
    expect(templateStartOf({ kind: 'topup', categories: ['salary'] })).toMatchObject({
      kind: 'topup',
      category: 'salary',
    })
    expect(templateStartOf({ kind: 'convert' })?.kind).toBe('payment')
    expect(templateStartOf(null)).toBeNull()
  })

  it('rejects a transfer to its own wallet', () => {
    const transfer = { ...draft, kind: 'transfer' as const, category: '', to: 'wallet' }
    expect(Object.keys(templateErrors(transfer, 'wallet'))).toEqual(['to'])
    expect(templateErrors({ ...transfer, to: 'other' }, 'wallet')).toEqual({})
  })

  describe('a stored template', () => {
    const asset = { symbol: 'THB', name: 'Baht', coingecko_id: null, chain: null, contract: null, quote: 'fiat' }
    const ident = { symbol: 'THB', name: 'Baht', coingecko_id: null, chain: null, contract: null }
    const price = { price_usd: '0.03', price_base: '2.7', price_base_currency: 'rub', fx_override: '90' }
    const stored = {
      payment: {
        meta: {
          id: 'x',
          timestamp: 't',
          comment: 'rent',
          categories: ['fixed', 'home'],
          flow_id: 'flow',
          contact_id: 'landlord',
        },
        store_id: 'wallet',
        asset,
        amount: '100',
        fee: { asset, amount: '1', price },
        to: { raw: 'Landlord' },
        details: { cash_breakdown: null, unit_price: '5', network: null, tx_hash: null },
        price,
      },
    } as unknown as Transaction

    it('is written back unchanged, in its own asset', () => {
      const input = templateToInput(templateDraftOf(stored), 'wallet', null, 'rub')
      expect(templateAssetOf(stored)).toEqual(asset)
      expect(input).toEqual({
        type: 'payment',
        common: {
          timestamp: null,
          comment: 'rent',
          categories: ['fixed', 'home'],
          flow_id: 'flow',
          contact_id: 'landlord',
          price_usd: '0.03',
          fx_override: '90',
          price: '2.7',
          price_currency: 'rub',
        },
        store_id: 'wallet',
        asset: ident,
        amount: '100',
        fee: { asset: ident, amount: '1' },
        to: { raw: 'Landlord' },
        details: { cash_breakdown: null, unit_price: '5', network: null, tx_hash: null },
      })
    })

    it('keeps the list of categories until another one is picked, and drops the other side with the kind', () => {
      const edited = templateDraftOf(stored)
      edited.category = 'daily'
      expect(templateToInput(edited, 'wallet', null, 'rub').common.categories).toEqual(['daily'])
      templateRetyped(edited, 'topup')
      expect(templateToInput(edited, 'wallet', null, 'rub')).toMatchObject({ type: 'topup', from: null, asset: ident })
    })
  })
})
