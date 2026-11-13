import { describe, expect, it } from 'vitest'

import { ruPlural } from './plural'

describe('ruPlural', () => {
  it('picks one, few and many', () => {
    const forms = (count: number) => ruPlural(count, 3)
    expect([1, 21, 101].map(forms)).toEqual([0, 0, 0])
    expect([2, 3, 4, 22].map(forms)).toEqual([1, 1, 1, 1])
    expect([5, 11, 12, 14, 19, 25, 100].map(forms)).toEqual([2, 2, 2, 2, 2, 2, 2])
  })

  it('keeps slot 0 for a message asked without a number', () => {
    expect(ruPlural(0, 3)).toBe(0)
  })

  it('leaves a two-form message to the plain singular/plural split', () => {
    expect([ruPlural(1, 2), ruPlural(2, 2), ruPlural(5, 2)]).toEqual([0, 1, 1])
  })
})
