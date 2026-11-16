import { describe, expect, it } from 'vitest'

import { emptyForm } from './form-state'
import { applyPlan, type PlanFields } from './apply-plan'

const asset = (symbol: string): AssetRef =>
  ({ symbol, name: symbol, coingecko_id: null, chain: null, contract: null }) as AssetRef
const OFFERED: TxKind[] = ['topup', 'payment', 'transfer', 'convert']

describe('applyPlan', () => {
  it('a plan with both sides becomes an exchange with all four fields filled', () => {
    const form = emptyForm('store', 'transfer')
    const plan: PlanFields = { from: asset('USDT'), to: asset('BTC'), amount: '4500', quantity: '0.05', price: '' }
    expect(applyPlan(form, plan, OFFERED)).toBe(true)
    expect(form.kind).toBe('convert')
    expect(form.fromAsset?.symbol).toBe('USDT')
    expect(form.fromAmount).toBe('4500')
    expect(form.toAsset?.symbol).toBe('BTC')
    expect(form.toAmount).toBe('0.05')
  })

  it('leaves the received amount empty when the plan has no quantity', () => {
    const form = emptyForm('store', 'transfer')
    applyPlan(form, { from: asset('RUB'), to: asset('USDT'), amount: '1000', price: '' }, OFFERED)
    expect(form.kind).toBe('convert')
    expect(form.toAmount).toBe('')
  })

  it('a plan with only a source stays a top-up', () => {
    const form = emptyForm('store', 'transfer')
    applyPlan(form, { from: asset('USD'), to: null, amount: '10', price: '1' }, OFFERED)
    expect(form.kind).toBe('topup')
    expect(form.amount).toBe('10')
    expect(form.price).toBe('1')
  })

  it('never lands on a trade kind', () => {
    const form = emptyForm('store', 'transfer')
    applyPlan(form, { from: asset('USDT'), to: asset('BTC'), amount: '1', price: '' }, ['topup'])
    expect(form.kind).not.toBe('trade_spot')
  })
})
