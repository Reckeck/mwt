import { describe, expect, it } from 'vitest'

import { i18n } from '@/shared/config/i18n'

import { KM_MAX } from '../vehicle-service.constants'
import { kmError } from './mileage'

const range = () => i18n.global.t('validation.range', { min: 0, max: KM_MAX })

describe('mileage', () => {
  it('passes the limits', () => {
    expect(kmError('0')).toBeNull()
    expect(kmError(String(KM_MAX))).toBeNull()
  })
  it('shows the range one past each limit', () => {
    expect(kmError('-1')).toBe(range())
    expect(kmError(String(KM_MAX + 1))).toBe(range())
  })
  it('wants a whole number', () => {
    expect(kmError('10.5')).not.toBeNull()
  })
})
