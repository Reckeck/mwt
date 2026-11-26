import { describe, expect, it } from 'vitest'

import { showBack } from './show-back'

describe('showBack', () => {
  it('is off at a section root', () => {
    expect(showBack(true, 'transactions')).toBe(false)
    expect(showBack(true, 'dashboard')).toBe(false)
  })
  it('is on for a section tab and a drill-down on a phone', () => {
    expect(showBack(true, 'transactions-calendar')).toBe(true)
    expect(showBack(true, 'wallet')).toBe(true)
    expect(showBack(true, 'money-month')).toBe(true)
  })
  it('is off on a desktop', () => {
    expect(showBack(false, 'wallet')).toBe(false)
  })
})
