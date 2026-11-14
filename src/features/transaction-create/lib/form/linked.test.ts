import { describe, expect, it } from 'vitest'
import { emptyForm } from './form-state'
import type { TransactionFormState } from './form-state'
import { formToInputs } from './linked'
import { missingFields } from './validation'

const usdt: AssetRef = {
  symbol: 'USDT',
  name: 'Tether',
  coingecko_id: null,
  chain: null,
  contract: null,
  quote: 'USDT',
}
const rub: AssetRef = { symbol: 'RUB', name: 'Ruble', coingecko_id: null, chain: 'fiat', contract: null, quote: 'RUB' }

function exchange(over: Partial<TransactionFormState> = {}): TransactionFormState {
  return {
    ...emptyForm('store-1', 'convert'),
    to: { store: 'store-2' },
    fromAsset: usdt,
    toAsset: rub,
    fromAmount: '100',
    toAmount: '9000',
    feeAmount: '1',
    rate: '90',
    ...over,
  }
}

const split = (over: Partial<TransactionFormState> = {}) =>
  exchange({ extra: [{ id: 'a', to: { store: 'store-3' }, receive: '3000' }], toShare: '6000', ...over })

type Convert = Extract<TransactionInput, { type: 'convert' }>
const parts = (form: TransactionFormState) => formToInputs(form) as Convert[]

describe('formToInputs for an exchange', () => {
  it('is one input with the fee when there are no extra rows', () => {
    const out = parts(exchange())
    expect(out).toHaveLength(1)
    expect(out[0].fee).not.toBeNull()
    expect(out[0].to.amount).toBe('9000')
  })

  it('splits the given amount in proportion and keeps the sum exact', () => {
    const out = parts(split())
    expect(out).toHaveLength(2)
    expect(out[0].to.amount).toBe('6000')
    expect(out[1].to.amount).toBe('3000')
    expect(Number(out[0].from.amount) + Number(out[1].from.amount)).toBe(100)
    expect(out[1].from.amount).toBe('33.33')
  })

  it('books the fee on the first part only', () => {
    const out = parts(split())
    expect(out[0].fee).not.toBeNull()
    expect(out[1].fee).toBeNull()
  })

  it('keeps the rate override on the first part only', () => {
    const out = parts(split())
    expect(out[0].rate_override).toBe('90')
    expect(out[1].rate_override).toBeNull()
  })

  it('gives the first destination the remainder when the share is empty', () => {
    const out = parts(split({ toShare: '' }))
    expect(out[0].to.amount).toBe('6000')
    expect(out[1].to.amount).toBe('3000')
  })

  it('is null when the shares do not add up to the total', () => {
    expect(formToInputs(split({ toShare: '5000' }))).toBeNull()
  })
})

describe('missingFields for an exchange', () => {
  it('flags shares that do not add up', () => {
    expect(missingFields(split({ toShare: '5000' }))).toContain('shares')
    expect(missingFields(split())).not.toContain('shares')
    expect(missingFields(split({ toShare: '' }))).not.toContain('shares')
  })

  it('asks for a via only once the field was opened', () => {
    expect(missingFields(exchange())).not.toContain('via')
    expect(missingFields(exchange({ viaOpen: true }))).toContain('via')
    expect(missingFields(exchange({ viaOpen: true, via: { store: 'store-9' } }))).not.toContain('via')
  })
})
