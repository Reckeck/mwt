import { afterEach, describe, expect, it } from 'vitest'

import { groupByDay } from './group-by-day'

const originalZone = process.env.TZ
afterEach(() => {
  if (originalZone === undefined) delete process.env.TZ
  else process.env.TZ = originalZone
})

const row = (id: string, time: string, valueUsd?: string) => ({ id, time, value_usd: valueUsd }) as CryptoOpRow

describe('groupByDay', () => {
  it('groups by the local day, the one the time in the row shows', () => {
    process.env.TZ = 'Europe/Moscow'
    // 23:30 UTC on Nov 15 is 02:30 on Nov 16 in UTC+3; 20:00 UTC on Nov 15 is 23:00 on Nov 15.
    const days = groupByDay([row('late', '2026-11-15T23:30:00Z'), row('earlier', '2026-11-15T20:00:00Z')])
    expect(days.map((day) => [day.date, day.rows.map((entry) => entry.id)])).toEqual([
      ['2026-11-16', ['late']],
      ['2026-11-15', ['earlier']],
    ])
  })

  it('sums the day exactly, without float noise', () => {
    const [day] = groupByDay([row('one', '2026-11-15T10:00:00Z', '0.1'), row('two', '2026-11-15T09:00:00Z', '0.2')])
    expect(day.total).toBe('0.3')
  })

  it('leaves the total empty when nothing in the day has a price', () => {
    const [day] = groupByDay([row('none', '2026-11-15T10:00:00Z')])
    expect(day.total).toBeNull()
  })

  it('skips a value that is not a number instead of poisoning the total', () => {
    const [day] = groupByDay([row('bad', '2026-11-15T10:00:00Z', 'n/a'), row('ok', '2026-11-15T09:00:00Z', '2.5')])
    expect(day.total).toBe('2.5')
  })
})
