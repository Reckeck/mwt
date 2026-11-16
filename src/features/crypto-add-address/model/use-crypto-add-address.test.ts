import { afterEach, describe, expect, it, vi } from 'vitest'

import { notFuture } from './use-crypto-add-address'

const originalZone = process.env.TZ
afterEach(() => {
  if (originalZone === undefined) delete process.env.TZ
  else process.env.TZ = originalZone
  vi.useRealTimers()
})

describe('notFuture', () => {
  it("accepts the user's today in the first hours after local midnight east of Greenwich", () => {
    process.env.TZ = 'Europe/Moscow'
    vi.useFakeTimers()
    // 01:30 on Nov 16 in UTC+3, still Nov 15 in UTC.
    vi.setSystemTime(new Date('2026-11-15T22:30:00Z'))
    expect(notFuture('2026-11-16')).toBe(true)
    expect(notFuture('2026-11-17')).toBe(false)
  })

  it('refuses tomorrow late in the evening west of Greenwich', () => {
    process.env.TZ = 'America/Los_Angeles'
    vi.useFakeTimers()
    // 20:00 on Nov 15 in UTC-8, already Nov 16 in UTC.
    vi.setSystemTime(new Date('2026-11-16T04:00:00Z'))
    expect(notFuture('2026-11-15')).toBe(true)
    expect(notFuture('2026-11-16')).toBe(false)
  })
})
