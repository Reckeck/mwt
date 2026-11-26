import { describe, expect, it } from 'vitest'

import { i18n } from '@/shared/config/i18n'

import { payDayError } from './pay-day'

const range = () => i18n.global.t('validation.range', { min: 1, max: 31 })

describe('pay day', () => {
  it('passes the first and the last day of the month', () => {
    expect(payDayError('1')).toBeNull()
    expect(payDayError('31')).toBeNull()
  })
  it('shows the range one past each limit', () => {
    expect(payDayError('0')).toBe(range())
    expect(payDayError('32')).toBe(range())
  })
  it('asks for a day and for a whole one', () => {
    expect(payDayError('')).not.toBeNull()
    expect(payDayError('1.5')).not.toBeNull()
  })
})
