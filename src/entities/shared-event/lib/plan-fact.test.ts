import { describe, expect, it } from 'vitest'

import { diffText, diffTone, negate, planFact, qtyText, unitPrice } from './plan-fact'

const item = (cancelled = false): EventItem => ({
  id: 'i',
  title: 'Пиво',
  unit: 'шт',
  category_id: null,
  plan: null,
  purchases: [],
  cancelled,
  split: { equal: { among: [] } },
})

const view_ = (patch: Partial<ItemView>): ItemView => ({
  item_id: 'i',
  cost: '0',
  qty: '0',
  plan_total: null,
  plan_qty: null,
  diff_money: '0',
  diff_qty: '0',
  bought: false,
  shares: [],
  ...patch,
})

describe('planFact', () => {
  it('an unbought plan stays planned, without differences', () => {
    const fact = planFact(item(), view_({ plan_total: '1200', plan_qty: '6' }))
    expect(fact.state).toBe('planned')
    expect(fact.diffMoney).toBeNull()
  })

  it('a bought plan compares money and quantity', () => {
    const fact = planFact(
      item(),
      view_({
        plan_total: '1200',
        plan_qty: '6',
        bought: true,
        cost: '1800',
        qty: '8',
        diff_money: '600',
        diff_qty: '2',
      }),
    )
    expect(fact.state).toBe('bought')
    expect(qtyText(fact, 'шт', undefined, 'из')).toBe('8 из 6 шт')
    expect(diffText(fact.diffMoney ?? '0', (sum) => `${sum} ₽`)).toBe('+600 ₽')
  })

  it('a purchase with no plan is unplanned and has no difference', () => {
    const fact = planFact(item(), view_({ bought: true, cost: '500', qty: '1' }))
    expect(fact.state).toBe('unplanned')
    expect(fact.diffMoney).toBeNull()
    expect(qtyText(fact, 'уп')).toBe('1 уп')
  })

  it('a cancelled item is cancelled whatever was bought', () => {
    expect(planFact(item(true), view_({ plan_total: '10', plan_qty: '1' })).state).toBe('cancelled')
  })
})

describe('diffText', () => {
  it('writes the sign with a real minus and zero plainly', () => {
    const money = (sum: string) => `${sum} ₽`
    expect(diffText('-200', money)).toBe('−200 ₽')
    expect(diffText('0.00', money)).toBe('0 ₽')
  })
})

describe('diffTone', () => {
  it('overspending is over, saving is under', () => {
    expect(diffTone('600')).toBe('over')
    expect(diffTone('-1')).toBe('under')
    expect(diffTone('0')).toBe('even')
    expect(diffTone(null)).toBe('even')
  })
})

describe('unitPrice', () => {
  it('divides the total by the quantity', () => {
    expect(unitPrice('1800', '8')).toBe('225')
    expect(unitPrice('100', '0')).toBeNull()
  })
})

describe('negate', () => {
  it('flips the sign and keeps zero plain', () => {
    expect(negate('5540')).toBe('-5540')
    expect(negate('-1.5')).toBe('1.5')
    expect(negate('0.00')).toBe('0')
  })
})
