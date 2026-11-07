import { describe, expect, it } from 'vitest'

import { createActivityThrottle, minutesError } from './autolock'

describe('createActivityThrottle', () => {
  it('sends the first activity and then at most once per interval', () => {
    let clock = 1_000
    const shouldSend = createActivityThrottle(() => clock, 30_000)
    expect(shouldSend()).toBe(true)
    clock += 29_999
    expect(shouldSend()).toBe(false)
    clock += 1
    expect(shouldSend()).toBe(true)
    clock += 5_000
    expect(shouldSend()).toBe(false)
  })

  it('sends when the clock went backwards', () => {
    let clock = 100_000
    const shouldSend = createActivityThrottle(() => clock, 30_000)
    expect(shouldSend()).toBe(true)
    clock = 10_000
    expect(shouldSend()).toBe(true)
  })
})

describe('minutesError', () => {
  it('accepts whole minutes from 5 to 60', () => {
    for (const value of ['5', '15', ' 60 ']) expect(minutesError(value)).toBeNull()
  })

  it('names the range for anything else', () => {
    for (const value of ['', '4', '61', '90', '7.5', 'abc']) expect(minutesError(value)).toContain('5')
  })
})
