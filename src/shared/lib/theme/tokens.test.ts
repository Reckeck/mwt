import { describe, expect, it } from 'vitest'
import { getTheme } from './registry'
import { assertCompleteTheme, THEME_TOKENS } from './tokens'

function fullTokens(): Record<string, string> {
  return Object.fromEntries(THEME_TOKENS.map((token) => [token, '#000']))
}

describe('THEME_TOKENS', () => {
  it('has no font token (TK5) and no port, node, syntax token', () => {
    for (const name of THEME_TOKENS) {
      expect(name.startsWith('--font')).toBe(false)
      expect(name).not.toMatch(/^--(port|node|syntax)-/)
    }
  })

  it('has unique names, the four money tokens and the radius', () => {
    expect(new Set(THEME_TOKENS).size).toBe(THEME_TOKENS.length)
    expect(THEME_TOKENS).toEqual(
      expect.arrayContaining([
        '--money-income',
        '--money-expense',
        '--money-transfer',
        '--money-in-transit',
        '--radius',
      ]),
    )
  })
})

describe('assertCompleteTheme', () => {
  it('accepts a theme with every token', () => {
    expect(() => assertCompleteTheme({ id: 'x', tokens: fullTokens() })).not.toThrow()
  })

  it('names the missing token', () => {
    const tokens = fullTokens()
    delete tokens['--money-transfer']
    expect(() => assertCompleteTheme({ id: 'x', tokens })).toThrow('--money-transfer')
  })
})

describe('registry', () => {
  it('returns undefined for an unknown id', () => {
    expect(getTheme('nope')).toBeUndefined()
  })
})
