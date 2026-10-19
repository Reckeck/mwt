import type { RouteLocationRaw } from 'vue-router'

import { i18n } from '@/shared/config/i18n'

/** Put off: in ten minutes, in an hour, tomorrow morning. */
export const SNOOZES = ['10m', '1h', 'tomorrow'] as const
export type SnoozeChoice = (typeof SNOOZES)[number]

export function snoozeUntil(choice: SnoozeChoice): string {
  const at = new Date()
  if (choice === '10m') at.setMinutes(at.getMinutes() + 10)
  else if (choice === '1h') at.setHours(at.getHours() + 1)
  else {
    at.setDate(at.getDate() + 1)
    at.setHours(9, 0, 0, 0)
  }
  return at.toISOString()
}

/** When it comes next: its date, or the end of its snooze. */
export const dueOf = (reminder: Reminder) =>
  reminder.snoozed_until && reminder.snoozed_until > reminder.due_at ? reminder.snoozed_until : reminder.due_at

export const overdue = (reminder: Reminder) => !reminder.done && new Date(dueOf(reminder)).getTime() < Date.now()

export function repeatLabel(repeat: ReminderRepeat): string {
  const t = i18n.global.t
  return `${t('reminder.every')} ${repeat.every > 1 ? `${repeat.every} ` : ''}${t(`reminder.unit.${repeat.unit}`, repeat.every)}`
}

/** Where an automatic reminder comes from: the wallet, the goal, the contact. */
export function sourceLink(source: ReminderSource): RouteLocationRaw | null {
  switch (source.kind) {
    case 'goal_saving':
    case 'goal_deadline':
      return { name: 'planning-goals' }
    case 'debt':
      return { name: 'transactions-contacts' }
    case 'rent':
    case 'property_tax':
    case 'interest_tax':
      return null
    default:
      return { name: 'wallet', params: { id: source.id } }
  }
}

export type ReminderGroup = 'overdue' | 'today' | 'later' | 'done'

/** Which heading a reminder goes under: what is late first, then today, then the rest. */
export function groupOf(reminder: Reminder, now = new Date()): ReminderGroup {
  if (reminder.done) return 'done'
  const due = new Date(dueOf(reminder))
  if (due.getTime() < now.getTime()) return 'overdue'
  return due.toDateString() === now.toDateString() ? 'today' : 'later'
}
