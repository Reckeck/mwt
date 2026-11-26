import { describe, expect, it } from 'vitest'

import { i18n } from '@/shared/config/i18n'

import { rateError } from './rate-error'

const t = (key: string, values: Record<string, unknown>) => i18n.global.t(key, values)

describe('freedom rate', () => {
  it('passes blank, 0 and 100', () => {
    expect(rateError('', t)).toBeNull()
    expect(rateError('0', t)).toBeNull()
    expect(rateError('100', t)).toBeNull()
  })
  it('shows the range message above 100', () => {
    expect(rateError('100.01', t)).toBe(t('validation.range', { min: 0, max: 100 }))
  })
  it('refuses a negative rate', () => {
    expect(rateError('-1', t)).not.toBeNull()
  })
})
