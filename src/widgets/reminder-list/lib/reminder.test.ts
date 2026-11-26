import { describe, expect, it } from 'vitest'

import { groupOf, groupReminders, repeatLabel } from './reminder'

const NOW = new Date('2026-11-26T12:00:00')
const make = (id: string, due_at: string, extra: Partial<Reminder> = {}) =>
  ({ id, text: id, due_at, done: false, muted: false, source: null, snoozed_until: null, ...extra }) as Reminder

describe('reminder groups', () => {
  it('puts a late reminder first, then today, later and done', () => {
    const groups = groupReminders(
      [
        make('later', '2026-12-01T09:00:00'),
        make('done', '2026-11-20T09:00:00', { done: true }),
        make('late', '2026-11-25T09:00:00'),
        make('today', '2026-11-26T18:00:00'),
      ],
      NOW,
    )
    expect(groups.map((group) => group.key)).toEqual(['overdue', 'today', 'later', 'done'])
  })

  it('hides a muted automatic reminder', () => {
    const muted = make('auto', '2026-12-01T09:00:00', {
      muted: true,
      source: { kind: 'goal_saving', id: 'x' } as ReminderSource,
    })
    expect(groupReminders([muted], NOW)).toEqual([])
  })

  it('counts the end of a snooze as the due time', () => {
    const snoozed = make('s', '2026-11-25T09:00:00', { snoozed_until: '2026-11-27T09:00:00' })
    expect(groupOf(snoozed, NOW)).toBe('later')
  })
})

/** The test locale is ru (src/test/setup.ts). */
describe('repeatLabel', () => {
  it('agrees the determiner with the unit', () => {
    expect(repeatLabel({ every: 1, unit: 'month' } as ReminderRepeat)).toBe('каждый месяц')
    expect(repeatLabel({ every: 1, unit: 'week' } as ReminderRepeat)).toBe('каждую неделю')
  })

  it('counts in Russian plural forms', () => {
    expect(repeatLabel({ every: 2, unit: 'month' } as ReminderRepeat)).toBe('каждые 2 месяца')
    expect(repeatLabel({ every: 5, unit: 'month' } as ReminderRepeat)).toBe('каждые 5 месяцев')
    expect(repeatLabel({ every: 11, unit: 'day' } as ReminderRepeat)).toBe('каждые 11 дней')
    expect(repeatLabel({ every: 21, unit: 'day' } as ReminderRepeat)).toBe('каждый 21 день')
  })
})
