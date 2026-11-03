import { describe, expect, it } from 'vitest'

import { fieldsOfReminder, reminderBaseOf, reminderInputOfFields } from './reminder-fields'

const reminder = (patch: Partial<Reminder> = {}): Reminder =>
  ({
    id: 'r1',
    text: 'Pay rent',
    due_at: new Date(2026, 10, 10, 8, 30).toISOString(),
    done: false,
    repeat: { every: 2, unit: 'week' },
    source: null,
    pay: undefined,
    ...patch,
  }) as Reminder

describe('reminder fields', () => {
  it('round-trips text, moment, repeat and pay', () => {
    const stored = reminder({ pay: { amount: '10' } as unknown as ReminderPay })
    const input = reminderInputOfFields(fieldsOfReminder(stored))
    expect(input).toEqual({
      text: 'Pay rent',
      due_at: stored.due_at,
      repeat: { every: 2, unit: 'week' },
      pay: stored.pay,
    })
  })

  it('a one-off reminder has no repeat', () => {
    expect(reminderInputOfFields(fieldsOfReminder(reminder({ repeat: null }))).repeat).toBeNull()
  })

  it('the base is written the way a save writes it (minute precision)', () => {
    const stored = reminder({ due_at: new Date(2026, 10, 10, 8, 30, 45).toISOString() })
    expect(reminderBaseOf(stored).due_at).toBe(new Date(2026, 10, 10, 8, 30).toISOString())
  })
})
