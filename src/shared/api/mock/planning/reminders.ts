/** Commands: reminders and price alerts. */

import { alerts, fail, reminders, remove, replace, uuid } from './state'
import type { PlanningMockContext } from './state'
import { autoUtilityReminders } from './auto-reminders'

/** Planning commands: Reminders and price alerts. `undefined` when `cmd` is not one. */
export function planReminders(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
  _settings: Settings,
): unknown {
  const body = args?.input as Record<string, unknown> | undefined
  switch (cmd) {
    case 'list_reminders':
      return [...reminders, ...autoUtilityReminders(ctx)].sort((left, right) => left.due_at.localeCompare(right.due_at))
    case 'create_reminder': {
      const input = body as ReminderInput
      const reminder: Reminder = {
        id: uuid(),
        text: input.text,
        due_at: input.due_at,
        done: false,
        repeat: input.repeat ?? null,
        source: null,
        muted: false,
        user_edited: false,
        snoozed_until: null,
        source_key: '',
      }
      reminders.push(reminder)
      return reminder
    }
    case 'update_reminder': {
      const input = body as UpdateReminderInput
      const current = reminders.find((entry) => entry.id === input.id) ?? fail('not found', 'MISSING')
      return replace(reminders, input.id, {
        ...current,
        ...input.reminder,
        repeat: input.reminder.repeat ?? null,
        snoozed_until: null,
      })
    }
    case 'delete_reminder': {
      // An automatic one is muted, not removed: its source would make it again.
      const reminder = reminders.find((entry) => entry.id === args?.id) ?? fail('not found', 'MISSING')
      if (reminder.source) Object.assign(reminder, { repeat: null, done: true, snoozed_until: null, muted: true })
      else remove(reminders, reminder.id)
      return {}
    }
    case 'set_reminder_done': {
      const reminder = reminders.find((entry) => entry.id === args?.id) ?? fail('not found', 'MISSING')
      reminder.snoozed_until = null
      if (args?.done && reminder.repeat) {
        // Like `next_occurrence`: counted from the due date, past now.
        const anchor = new Date(reminder.due_at)
        const { every, unit } = reminder.repeat
        for (let count = 1; count < 10_000; count += 1) {
          const next = new Date(anchor)
          if (unit === 'day') next.setUTCDate(anchor.getUTCDate() + every * count)
          if (unit === 'week') next.setUTCDate(anchor.getUTCDate() + every * count * 7)
          if (unit === 'month') next.setUTCMonth(anchor.getUTCMonth() + every * count)
          if (unit === 'year') next.setUTCFullYear(anchor.getUTCFullYear() + every * count)
          if (next.getTime() > Date.now()) {
            reminder.due_at = next.toISOString()
            break
          }
        }
      } else reminder.done = Boolean(args?.done)
      return reminder
    }
    case 'finish_reminder_series': {
      const reminder = reminders.find((entry) => entry.id === args?.id) ?? fail('not found', 'MISSING')
      Object.assign(reminder, { repeat: null, done: true, snoozed_until: null })
      return reminder
    }
    case 'snooze_reminder': {
      const input = body as SnoozeReminderInput
      const reminder = reminders.find((entry) => entry.id === input.id) ?? fail('not found', 'MISSING')
      reminder.snoozed_until = input.until
      return reminder
    }
    case 'list_price_alerts':
      return alerts
    case 'create_price_alert': {
      const input = body as PriceAlertInput
      const symbol = input.asset.symbol.toUpperCase()
      const alert: PriceAlert = {
        id: uuid(),
        asset: { ...input.asset, symbol, quote: symbol },
        condition: input.condition,
        price: input.price,
        currency: input.currency,
        active: input.active,
        triggered_at: null,
        ...(input.store_id ? { store_id: input.store_id } : {}),
      }
      alerts.push(alert)
      return alert
    }
    case 'update_price_alert': {
      const input = body as UpdatePriceAlertInput
      const symbol = input.alert.asset.symbol.toUpperCase()
      return replace(alerts, input.id, {
        ...input.alert,
        asset: { ...input.alert.asset, symbol, quote: symbol },
        triggered_at: null,
      })
    }
    case 'delete_price_alert':
      remove(alerts, String(args?.id))
      return {}
    default:
      return undefined
  }
}
