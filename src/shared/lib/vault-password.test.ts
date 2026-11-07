import { describe, expect, it } from 'vitest'

import { cryptoPasswordOk, passwordStrength, pinOk } from './vault-password'

const STRONG = 'Tr0ub4dor&3-Horse-Battery!'

describe('vault password rules', () => {
  it('takes a new PIN of six digits or more', () => {
    expect(pinOk('123456')).toBe(true)
    for (const refused of ['', '1234', '12345', '12345a', '123 456']) expect(pinOk(refused)).toBe(false)
  })

  it('grades the level-3 password like the core', () => {
    for (const weak of ['short1A!', 'aaaaaaaaaaaaaaaaaaaa', 'abcdefghijklmnopqrstu', '12345678901234567890'])
      expect(passwordStrength(weak).strength).toBe('weak')
    expect(passwordStrength('correcthorsebatter').strength).toBe('medium')
    expect(passwordStrength(STRONG).strength).toBe('strong')
    expect(passwordStrength('correct horse battery staple').strength).toBe('strong')
    expect(passwordStrength('fifteen chars!!').long_enough).toBe(false)
    expect(passwordStrength('sixteen chars!!!').long_enough).toBe(true)
    expect(cryptoPasswordOk('correcthorsebatter')).toBe(true)
    expect(cryptoPasswordOk('aaaaaaaaaaaaaaaaaaaa')).toBe(false)
  })
})
