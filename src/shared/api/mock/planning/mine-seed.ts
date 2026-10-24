/** The rest of «Мои» in the demo vault (Auto-1 · 1): a transfer repeat, the salary reminder and the BTC alert. */

import { syncWorkflow } from './automation-workflow'
import { alerts, automations, draftOf, iso, now, reminders, uuid } from './state'

/** `transfer`: a booked operation from the bank to the exchange the repeat copies. */
export function seedMine(transfer: Transaction) {
  const start = now()
  const automation: Automation = {
    id: uuid(),
    name: 'Перевод на Bybit',
    active: true,
    preset: null,
    schedule: { monthly: { day: 1 } },
    starts_at: iso(start),
    template: draftOf(transfer),
    last_planned_at: null,
    created_at: iso(start),
    amount_mode: 'fixed',
    workday_shift: 'none',
    property_id: null,
    estimate: null,
  }
  automations.push(automation)
  syncWorkflow(automation)
  const due = new Date(start)
  due.setDate(due.getDate() + 1)
  due.setHours(9, 0, 0, 0)
  reminders.push({
    id: uuid(),
    text: 'Сегодня должна прийти зарплата',
    due_at: iso(due),
    done: false,
    repeat: null,
    source: null,
    muted: false,
    user_edited: false,
    snoozed_until: null,
    source_key: '',
  })
  alerts.push({
    id: uuid(),
    asset: { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin', chain: 'bitcoin', contract: null, quote: 'BTC' },
    condition: 'above',
    price: '90000',
    currency: 'usd',
    active: true,
    triggered_at: null,
  })
}
