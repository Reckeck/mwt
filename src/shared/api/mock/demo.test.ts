import { afterEach, describe, expect, it, vi } from 'vitest'

import { demoTransactions } from './demo'

describe('demoTransactions', () => {
  afterEach(() => vi.useRealTimers())

  // The demo is read through the current month: no day of the month may push its history out of it.
  it.each([
    '2026-11-01T00:50:00',
    '2026-11-01T13:40:00',
    '2026-11-02T10:00:00',
    '2026-10-31T23:50:00',
    '2026-02-28T12:00:00',
  ])('keeps every operation in the month of %s', (iso) => {
    const now = new Date(iso)
    vi.useFakeTimers()
    vi.setSystemTime(now)
    const month = `${now.getFullYear()}-${now.getMonth()}`
    const stamps = demoTransactions().map(
      (tx) => new Date((Object.values(tx)[0] as { meta: { timestamp: string } }).meta.timestamp),
    )
    // The two oldest exchanges are older than any month on purpose; the rest is the month's.
    const recent = stamps.filter((stamp) => now.getTime() - stamp.getTime() <= 240 * 3.6e6)
    expect(recent.length).toBeGreaterThanOrEqual(8)
    for (const stamp of recent) expect(`${stamp.getFullYear()}-${stamp.getMonth()}`).toBe(month)
    // The mock's analytics read the month off the UTC timestamp: the same month there, whenever local and UTC agree on it.
    const utcMonth = now.toISOString().slice(0, 7)
    if (`${now.getFullYear()}-${now.getMonth()}` === `${now.getUTCFullYear()}-${now.getUTCMonth()}`)
      for (const stamp of recent) expect(stamp.toISOString().slice(0, 7)).toBe(utcMonth)
  })
})
