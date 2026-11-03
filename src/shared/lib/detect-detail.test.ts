import { describe, expect, it } from 'vitest'

import { detectDetail, looksLikePhone } from './detect-detail'

describe('detectDetail phones', () => {
  it.each([
    '+79123456789',
    '+7 912 345-67-89',
    '8 912 345 67 89',
    '8 (912) 345-67-89',
    '8-912-345-67-89',
    '89123456789',
    '7 912 345 67 89',
    '79123456789',
  ])('%s is a phone', (input) => {
    expect(detectDetail(input)).toEqual({ kind: 'phone', network: null, value: '+79123456789' })
  })

  it('keeps an international number', () => {
    expect(detectDetail('+44 20 7946 0958')).toEqual({ kind: 'phone', network: null, value: '+442079460958' })
  })

  it('leaves bare ten digits ambiguous', () => {
    expect(looksLikePhone('9123456789')).toBeNull()
  })

  it('keeps a long account number an account', () => {
    expect(detectDetail('40817810099910004312').kind).toBe('account')
  })

  it('keeps a Luhn-valid card a card', () => {
    expect(detectDetail('4111 1111 1111 1111').kind).toBe('card')
  })

  it('treats empty as other', () => {
    expect(detectDetail('  ').kind).toBe('other')
  })
})
