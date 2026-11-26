import { describe, expect, it } from 'vitest'

import { hasContactSide, quickDebtEffect } from './debt-effect'

const wallet = (id: string): AccountRef => ({ store: id })
const contact: AccountRef = { contact: { contact_id: 'c1', account_id: null } }

describe('quickDebtEffect', () => {
  it('lends on money out to a contact and borrows on money in from one', () => {
    expect(quickDebtEffect(wallet('a'), contact, true)).toBe('lend')
    expect(quickDebtEffect(contact, wallet('a'), true)).toBe('borrow')
  })
  it('is none when it is not marked as a loan', () => {
    expect(quickDebtEffect(wallet('a'), contact, false)).toBeNull()
  })
  it('is none for wallet to wallet, which the core would refuse as a loan', () => {
    expect(quickDebtEffect(wallet('a'), wallet('b'), true)).toBeNull()
  })
})

describe('hasContactSide', () => {
  it('is true for exactly one contact side', () => {
    expect(hasContactSide(wallet('a'), contact)).toBe(true)
    expect(hasContactSide(wallet('a'), wallet('b'))).toBe(false)
    expect(hasContactSide(contact, contact)).toBe(false)
  })
})
