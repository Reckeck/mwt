import { describe, expect, it } from 'vitest'

import { FORMULA_VARIABLES, zoneKey } from './meter-formula'

describe('meter formula', () => {
  it('turns a zone name into a variable', () => {
    expect(zoneKey('  Peak  Night ')).toBe('peak_night')
    expect(zoneKey('day')).toBe('day')
  })

  it('reads both the Russian and the Latin names', () => {
    expect(FORMULA_VARIABLES.map(([, latin]) => latin)).toEqual(['current', 'previous', 'used', 'tariff', 'days'])
  })
})
