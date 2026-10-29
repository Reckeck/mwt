import { describe, expect, it } from 'vitest'

import { defaultPurchaseDate } from './purchase-date'

describe('defaultPurchaseDate', () => {
  it('takes today inside the range, the first day outside it', () => {
    expect(defaultPurchaseDate('2026-11-03', '2026-11-02', '2026-11-04')).toBe('2026-11-03')
    expect(defaultPurchaseDate('2026-10-20', '2026-11-02', '2026-11-04')).toBe('2026-11-02')
    expect(defaultPurchaseDate('2026-12-01', '2026-11-02', '2026-11-04')).toBe('2026-11-02')
  })

  it('treats a single day as a range of one and no date as today', () => {
    expect(defaultPurchaseDate('2026-11-02', '2026-11-02', null)).toBe('2026-11-02')
    expect(defaultPurchaseDate('2026-11-03', '2026-11-02', null)).toBe('2026-11-02')
    expect(defaultPurchaseDate('2026-11-03', null, null)).toBe('2026-11-03')
  })
})
