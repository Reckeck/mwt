import { i18n } from '@/shared/config/i18n'

export type ReminderGroup = 'overdue' | 'today' | 'later' | 'done'

/** When it comes next: its date, or the end of its snooze. */
export const dueOf = (reminder: Reminder) =>
  reminder.snoozed_until && reminder.snoozed_until > reminder.due_at ? reminder.snoozed_until : reminder.due_at

/** Which heading a reminder goes under: what is late first, then today, then the rest. */
export function groupOf(reminder: Reminder, now = new Date()): ReminderGroup {
  if (reminder.done) return 'done'
  const due = new Date(dueOf(reminder))
  if (due.getTime() < now.getTime()) return 'overdue'
  return due.toDateString() === now.toDateString() ? 'today' : 'later'
}

/**
 * «каждый месяц», «каждые 2 месяца», «каждые 5 месяцев»: Russian needs the
 * determiner to agree with the unit, so every unit carries the whole phrase
 * instead of «каждые» glued to a bare noun.
 */
export function repeatLabel(repeat: ReminderRepeat): string {
  const { t } = i18n.global
  if (repeat.every <= 1) return t(`reminder.repeatEvery.${repeat.unit}`)
  return t(`reminder.repeatEveryN.${repeat.unit}`, { count: repeat.every })
}

const GROUPS: ReminderGroup[] = ['overdue', 'today', 'later', 'done']

/** The visible reminders (an automatic one the user muted stays hidden), by time, under their headings. */
export function groupReminders(reminders: Reminder[], now = new Date()) {
  const visible = reminders
    .filter((reminder) => !(reminder.source && reminder.muted))
    .sort((left, right) => Number(left.done) - Number(right.done) || dueOf(left).localeCompare(dueOf(right)))
  return GROUPS.map((key) => ({ key, items: visible.filter((reminder) => groupOf(reminder, now) === key) })).filter(
    (group) => group.items.length,
  )
}
