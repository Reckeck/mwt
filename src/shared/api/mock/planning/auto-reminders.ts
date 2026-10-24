/** Automatic utility reminders, like `sync_auto_reminders` in Rust: shown, never stored. */

import { utilities } from './state'
import type { PlanningMockContext } from './state'

/** «Send the readings» comes this many days before the pay day. */
const METER_LEAD_DAYS = 3

/** The next `day` of a month at 09:00 local, this month or the next; clamped to the month's length. */
function nextMonthly(day: number): string {
  const now = new Date()
  const at = (year: number, month: number) => {
    const last = new Date(year, month + 1, 0).getDate()
    return new Date(year, month, Math.min(day, last), 9, 0, 0)
  }
  const here = at(now.getFullYear(), now.getMonth())
  return (here >= now ? here : at(now.getFullYear(), now.getMonth() + 1)).toISOString()
}

function auto(kind: ReminderSourceKind, store: Store, text: string, day: number): Reminder {
  return {
    id: `auto-${kind}-${store.id}`,
    text,
    due_at: nextMonthly(day),
    done: false,
    repeat: { every: 1, unit: 'month' },
    source: { kind, id: store.id },
    muted: false,
    user_edited: false,
    snoozed_until: null,
    source_key: `${kind}:${store.id}`,
    pay: null,
  }
}

/** «Pay the utilities» on the property's pay day, «Send the readings» before it when it has meters. */
export function autoUtilityReminders(ctx: PlanningMockContext): Reminder[] {
  return ctx.stores
    .filter((store) => store.kind === 'property' && !store.archived)
    .flatMap((store) => {
      const day = store.property?.utilities_pay_day
      const items = utilities.filter((item) => item.active && item.property_id === store.id)
      if (!day || !items.length) return []
      const out = [auto('utilities', store, `Оплатить ЖКХ: ${store.name}, ${day}-го`, day)]
      if (items.some((item) => 'metered' in item.kind)) {
        out.push(
          auto(
            'meter_readings',
            store,
            `Передать показания счётчиков: ${store.name}`,
            Math.max(1, day - METER_LEAD_DAYS),
          ),
        )
      }
      return out
    })
}
