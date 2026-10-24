/** How a «Мои» row reads (Auto-1 · 1): the name, when it fires next, what it will do. */

import type { LocationQuery, RouteLocationRaw } from 'vue-router'

import { entityLocation } from '@/shared/lib/entity-link'
import { formatDayMonthShort, formatMoney } from '@/shared/lib/format'

type Translate = (key: string, params?: Record<string, unknown>) => string

/** The chips above the list, in the frame's order; `all` shows everything. */
export const MINE_FILTERS = ['all', 'repeat', 'scheme', 'reminder', 'alert'] as const
export type MineFilter = (typeof MINE_FILTERS)[number]

export const isMineFilter = (value: unknown): value is MineFilter => MINE_FILTERS.includes(value as MineFilter)

/** «1 575 $», «45 000 ₽»: whole sums without kopecks, as on the frame. */
function sum(amount: string, currency: string, signed = false): string {
  const whole = Number.isInteger(Number(amount))
  return formatMoney(amount, { currency, signed, decimals: whole ? 0 : 2 })
}

/** «BTC ≥ 90 000 $» for an alert; every other kind keeps its own name. */
export function mineName(item: MineItem): string {
  if (item.kind !== 'alert' || !item.amount) return item.name
  const sign = item.condition === 'below' ? '≤' : '≥'
  return `${item.name} ${sign} ${sum(item.amount, item.currency ?? 'usd')}`
}

/** «25 ноя»; «по цене» for an alert, «по событию» for a scheme that waits for an operation. */
export function mineWhen(item: MineItem, t: Translate): string {
  if (item.next_at) return formatDayMonthShort(item.next_at)
  return item.kind === 'alert' ? t('automations.mine.byPrice') : t('automations.mine.byEvent')
}

/**
 * «+1 575 $ → Тинькофф», «45 000 ₽ Тинькофф → Bybit», «уведомление»; a scheme
 * says what its description says. « · ждёт «да»» when it asks first.
 */
export function mineDoes(item: MineItem, storeName: (id: string) => string, t: Translate): string {
  const asks = item.asks ? ` · ${t('automations.mine.asks')}` : ''
  if (item.notifies) return t('automations.mine.notify') + asks
  if (item.kind === 'scheme') return (item.description || t('automations.mine.scheme')) + asks
  if (!item.amount || !item.asset) return t('automations.mine.operation') + asks
  const currency = item.asset.symbol
  const from = item.from_store ? storeName(item.from_store) : ''
  const to = item.to_store ? storeName(item.to_store) : ''
  if (from && to) return `${sum(item.amount, currency)} ${from} → ${to}${asks}`
  const amount = sum(item.amount, currency, true)
  return (to ? `${amount} → ${to}` : `${amount} ${from}`.trim()) + asks
}

/** Where a row leads: a scheme or a repeat opens its card in place (Auto-1 · 3), a reminder and an alert their lists. */
export function mineRoute(item: MineItem, query: LocationQuery): RouteLocationRaw {
  if (item.kind === 'reminder') return { name: 'automations-reminders' }
  if (item.kind === 'alert') return { name: 'automations-alerts' }
  return entityLocation({ kind: 'automation', id: item.id }, query)
}

/** The ready-made starts of the empty «Мои» (Auto-1 · 2), in the frame's order. */
export const MINE_STARTERS = ['salary', 'save_share', 'low_balance', 'weekly_buy'] as const
export type MineStarter = (typeof MINE_STARTERS)[number]
