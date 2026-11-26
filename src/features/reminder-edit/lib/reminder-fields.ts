import { localDate } from '@/shared/lib/planning'

import { type WhenPreset, presetOf, repeatOf } from './when-presets'

/** The reminder form's fields as plain values (what the refs of `ReminderDialog` hold). */
export interface ReminderFields {
  text: string
  /** `YYYY-MM-DD`, local. */
  date: string
  /** `HH:MM`, local. */
  time: string
  preset: WhenPreset
  unit: RepeatUnit
  every: string
  pay: ReminderPay | null
}

const pad = (value: number) => String(value).padStart(2, '0')

/** The fields a stored reminder fills in the form; `null` is a new one (tomorrow, 09:00). */
export function fieldsOfReminder(reminder: Reminder | null): ReminderFields {
  const due = reminder ? new Date(reminder.due_at) : new Date(Date.now() + 86_400_000)
  return {
    text: reminder?.text ?? '',
    date: localDate(due),
    time: reminder ? `${pad(due.getHours())}:${pad(due.getMinutes())}` : '09:00',
    preset: presetOf(reminder?.repeat),
    unit: reminder?.repeat?.unit ?? 'month',
    every: String(reminder?.repeat?.every ?? 1),
    pay: reminder?.pay ?? null,
  }
}

/** The moment the date and time fields name; `null` while they do not name one. */
export function dueAtOfFields(fields: Pick<ReminderFields, 'date' | 'time'>): string | null {
  if (!fields.date || !fields.time) return null
  const parsed = new Date(`${fields.date}T${fields.time}`)
  return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString()
}

export function reminderInputOfFields(fields: ReminderFields): ReminderInput {
  return {
    text: fields.text.trim(),
    due_at: dueAtOfFields(fields) ?? '',
    repeat: repeatOf(fields.preset, { unit: fields.unit, every: Number(fields.every) }),
    pay: fields.pay ?? undefined,
  }
}

/**
 * The edit form's base. It goes through the form's own mapping so that it is written the way a
 * save writes it (minute precision, normalized repeat): an untouched field then never counts as
 * the user's change.
 */
export const reminderBaseOf = (reminder: Reminder): ReminderInput => reminderInputOfFields(fieldsOfReminder(reminder))
