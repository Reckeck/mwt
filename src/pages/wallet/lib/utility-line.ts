import type { Component } from 'vue'
import { Droplet, Gauge, House, Receipt, Wifi, Zap } from 'lucide-vue-next'

import { formatMoney, formatQuantity } from '@/shared/lib/format'

type T = (key: string, values?: Record<string, unknown>) => string

/** One row of the bill as Wallets-2 · 10 shows it: already worded. */
export interface UtilityLineView {
  line: UtilityBillLine
  icon: Component
  name: string
  /** «по счётчику · день / ночь», «фиксированная», «аренда · 10-го · автоматизация». */
  subtitle: string
  /** «412 → 418 м³», «13 395 / 5 120», «—». */
  reading: string
  /** «6 м³», «ждёт показаний», «—». */
  consumption: string
  /** «294 ₽», «≈ 1 270 ₽». */
  amount: string
  /** A meter not read this month: its amount is an estimate, «Внести» stands beside. */
  waiting: boolean
  /** Rent an automation pays: «авто», never in the bill's total. */
  automated: boolean
}

/** By what the item is called, else by its kind: a bolt, a drop, wifi, a house. */
function iconOf(item: UtilityItem): Component {
  const name = item.name.toLowerCase()
  if ('housing' in item.kind) return House
  if (/электр|свет|power|electr/.test(name)) return Zap
  if (/вод|water/.test(name)) return Droplet
  if (/интернет|internet|wi-?fi/.test(name)) return Wifi
  return 'metered' in item.kind ? Gauge : Receipt
}

const number = (value: string | null) => (value === null ? null : formatQuantity(value, 2))

/** The reading column: per zone the last known, else «previous → this month». */
function readingOf(line: UtilityBillLine, unit: string): string {
  if (line.zones.length)
    return line.zones.map((zone) => number(zone.reading ?? zone.previous_reading) ?? '—').join(' / ')
  const [previous, current] = [number(line.previous_reading), number(line.reading)]
  if (previous && current) return `${previous} → ${current} ${unit}`
  const known = current ?? previous
  return known ? `${known} ${unit}` : '—'
}

export function utilityLineView(line: UtilityBillLine, t: T): UtilityLineView {
  const { item } = line
  const money = (value: string) => formatMoney(value, { currency: item.currency, decimals: 0 })
  const base = { line, icon: iconOf(item), name: item.name, waiting: false, automated: false }
  if ('metered' in item.kind) {
    const { unit, tariff, formula, zones } = item.kind.metered
    const how = zones.length
      ? zones.join(' / ')
      : formula
        ? t('utilityTab.ownFormula')
        : t('utilityTab.tariff', { price: money(tariff), unit })
    const waiting = line.amount === null
    return {
      ...base,
      subtitle: `${t('utilityTab.metered')} · ${how}`,
      reading: readingOf(line, unit),
      consumption: waiting ? t('utilityTab.waiting') : `${number(line.consumption) ?? '0'} ${unit}`,
      amount: waiting ? (line.estimate ? `≈ ${money(line.estimate)}` : '—') : money(line.amount ?? '0'),
      waiting,
    }
  }
  const fixed = line.amount ? money(line.amount) : '—'
  if ('housing' in item.kind) {
    const { due_day, automation_id } = item.kind.housing
    const parts = [
      t('utilityTab.rent'),
      t('utilityTab.day', { day: due_day }),
      ...(automation_id ? [t('utilityTab.automation')] : []),
    ]
    return {
      ...base,
      subtitle: parts.join(' · '),
      reading: '—',
      consumption: '—',
      amount: fixed,
      automated: !!automation_id,
    }
  }
  return { ...base, subtitle: t('utilityTab.fixed'), reading: '—', consumption: '—', amount: fixed }
}

/** The phone's one line under the name: the subtitle and what the desktop shows in the columns. */
export function phoneSubtitle(view: UtilityLineView, t: T): string {
  if (view.waiting) return `${view.subtitle} · ${t('utilityTab.waiting')}`
  if (view.consumption !== '—') return `${view.subtitle} · ${view.consumption}`
  return `${view.subtitle} · ${view.amount}`
}
