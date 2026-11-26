import { describe, expect, it } from 'vitest'

import { noticeFor } from './biometric-notice'

describe('noticeFor', () => {
  it('tells a reset binding from an expired window', () => {
    expect(noticeFor('BIOMETRIC_INVALIDATED')).toBe('reset')
    expect(noticeFor('BIOMETRIC_PASSWORD_REQUIRED')).toBe('password')
  })

  it('reads every other failure as "did not work"', () => {
    for (const code of ['BIOMETRIC_CANCELLED', 'BIOMETRIC_FAILED', 'BIOMETRIC_UNAVAILABLE', 'IO', '']) {
      expect(noticeFor(code)).toBe('failed')
    }
  })
})
