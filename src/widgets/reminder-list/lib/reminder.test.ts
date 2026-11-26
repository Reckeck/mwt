import { describe, expect, it } from 'vitest'

import { groupOf, groupReminders } from './reminder'

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
