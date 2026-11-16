import { afterEach, describe, expect, it, vi } from 'vitest'

import { localDay } from './datetime'

const originalZone = process.env.TZ
function inZone(zone: string) {
  process.env.TZ = zone
}
afterEach(() => {
  if (originalZone === undefined) delete process.env.TZ
  else process.env.TZ = originalZone
  vi.useRealTimers()
})

describe('localDay', () => {
  it('is the local calendar day, not the UTC one, just after local midnight east of Greenwich', () => {
    inZone('Europe/Moscow')
    // 01:30 on Nov 16 in UTC+3 is still Nov 15 in UTC.
    expect(localDay(new Date('2026-11-15T22:30:00Z'))).toBe('2026-11-16')
  })

  it('is the local day late in the evening west of Greenwich', () => {
    inZone('America/Los_Angeles')
    // 20:00 on Nov 15 in UTC-8 is already Nov 16 in UTC.
    expect(localDay(new Date('2026-11-16T04:00:00Z'))).toBe('2026-11-15')
  })

  it('follows the system clock by default', () => {
    inZone('Europe/Moscow')
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-11-15T22:30:00Z'))
    expect(localDay()).toBe('2026-11-16')
  })
})

describe('formatCalendarDay', () => {
  it.each(['America/Los_Angeles', 'Europe/Moscow', 'Pacific/Auckland'])(
    'keeps the day and shows the year in %s',
    async (zone) => {
      inZone(zone)
      // A fresh module: the date formatters are cached with the zone they were first built in.
      vi.resetModules()
      const { formatCalendarDay } = await import('./datetime')
      const text = formatCalendarDay('2026-01-01')
      expect(text).toContain('2026')
      expect(text).toMatch(/\b0?1\b/)
      expect(text).not.toMatch(/\b31\b/)
    },
  )

  it('shows a dash for an empty day', async () => {
    const { formatCalendarDay } = await import('./datetime')
    expect(formatCalendarDay('')).toBe('—')
  })
})
