import { describe, expect, it } from 'vitest'

import { templateDraftOf, templateErrors, templateKindOf, templateToInput } from './template-input'

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
})
