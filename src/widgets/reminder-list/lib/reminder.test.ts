import { describe, expect, it } from 'vitest'

import { repeatLabel } from './reminder'

/** The test locale is ru (src/test/setup.ts). */
describe('repeatLabel', () => {
  it('agrees the determiner with the unit', () => {
    expect(repeatLabel({ every: 1, unit: 'month' } as ReminderRepeat)).toBe('каждый месяц')
    expect(repeatLabel({ every: 1, unit: 'week' } as ReminderRepeat)).toBe('каждую неделю')
  })

  it('counts in Russian plural forms', () => {
    expect(repeatLabel({ every: 2, unit: 'month' } as ReminderRepeat)).toBe('каждые 2 месяца')
    expect(repeatLabel({ every: 5, unit: 'month' } as ReminderRepeat)).toBe('каждые 5 месяцев')
    expect(repeatLabel({ every: 3, unit: 'day' } as ReminderRepeat)).toBe('каждые 3 дня')
    expect(repeatLabel({ every: 11, unit: 'day' } as ReminderRepeat)).toBe('каждые 11 дней')
    expect(repeatLabel({ every: 21, unit: 'day' } as ReminderRepeat)).toBe('каждый 21 день')
  })
})
