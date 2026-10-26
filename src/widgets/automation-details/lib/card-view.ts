/** How the automation's card reads (Auto-1 · 3): «Когда», «Если», «Сделать», «Следующий раз», the runs. */

import { formatDayMonthShort, formatPct, formatWeekday } from '@/shared/lib/format'
import { composeSums, type SumPart } from '@/shared/lib/money-parts'
import { scheduleText } from '@/shared/lib/planning'

type Translate = (key: string, params?: Record<string, unknown>) => string
export type Names = { store: (id: string) => string; category: (key: string) => string }

/** «1 575 $», «157,50 $»: whole sums without kopecks, as on the frame. */
export function cardSum(amount: string, currency: string, at?: string): Exclude<SumPart, { text: string }> {
  const whole = Number.isInteger(Number(amount))
  return { amount, currency, decimals: whole ? 0 : 2, at }
}

/** «25-го; в выходной — в пятницу перед ним»; «по событию» without a schedule. */
export function cardWhen(card: AutomationCard, t: Translate): string {
  const { schedule } = card
  if (!schedule) return t(card.trigger.kind === 'manual' ? 'automationCard.when.manual' : 'automationCard.when.event')
  const base =
    'monthly' in schedule ? t('automationCard.when.monthly', { day: schedule.monthly.day }) : scheduleText(schedule, t)
  return card.workday === 'none' ? base : `${base}; ${t(`automationCard.when.${card.workday}`)}`
}

/** «пришла «Зарплата» на Тинькофф», «по расписанию». */
export function cardIf(trigger: CardTrigger, names: Names, t: Translate): string {
  if (trigger.kind === 'schedule') return t('automationCard.if.schedule')
  if (trigger.kind === 'manual') return t('automationCard.if.manual')
  if (trigger.kind === 'other') return t('automationCard.if.other')
  const store = trigger.store ? names.store(trigger.store) : ''
  const kind = trigger.tx_kind === 'payment' ? 'payment' : 'topup'
  const key = `automationCard.if.${kind}${trigger.category ? 'Category' : ''}${store ? 'Store' : ''}`
  return t(key, { category: trigger.category ? names.category(trigger.category) : '', store })
}

const VERBS = ['transfer', 'topup', 'payment', 'convert', 'distribute', 'notify', 'reminder'] as const

/** One action: «перевести 10 % на Наличные», «записать 1 575 $ на Тинькофф», «напомнить: …». */
function actionText(action: CardAction, names: Names, t: Translate): SumPart[] {
  const { money } = action
  const { mark, parts } = composeSums()
  const from = money.from_store ? names.store(money.from_store) : ''
  const to = money.to_store ? names.store(money.to_store) : ''
  const sum = action.percent
    ? formatPct(action.percent, { decimals: Number.isInteger(Number(action.percent)) ? 0 : 1 })
    : money.amount && money.currency
      ? mark(cardSum(money.amount, money.currency))
      : ''
  const verb = (VERBS as readonly string[]).includes(action.kind) ? action.kind : 'other'
  const text = t(`automationCard.do.${verb}`, { sum, from, to, title: action.title })
    .replace(/\s+/g, ' ')
    .replace(/\s+([,;])/g, '$1')
    .trim()
  return parts(text)
}

/** Every action of the card, «; » between them. */
export function cardDo(card: AutomationCard, names: Names, t: Translate): SumPart[] {
  if (!card.actions.length) return [{ text: t('automationCard.do.nothing') }]
  return card.actions.flatMap((action, index): SumPart[] => [
    ...(index ? [{ text: '; ' }] : []),
    ...actionText(action, names, t),
  ])
}

/** «25 ноя (среда)». */
export function cardNext(iso: string | null, t: Translate): string {
  if (!iso) return t('automationCard.next.none')
  return `${formatDayMonthShort(iso)} (${formatWeekday(new Date(iso), 'long')})`
}

/** How a run ended, and the tone it is said in. */
export type RunTone = 'pos' | 'wrn' | 'neg' | 'mut'
export function statusTone(status: JournalStatus): RunTone {
  if (status === 'done') return 'pos'
  if (status === 'asking' || status === 'waiting' || status === 'partial') return 'wrn'
  if (status === 'failed' || status === 'rejected') return 'neg'
  return 'mut'
}

/** «157,50 $ → Наличные», «1 575 $ с Тинькофф», or the run's note. */
export function runMoney(entry: JournalEntry, names: Names): SumPart[] {
  const { money } = entry
  const sum = money.amount && money.currency ? cardSum(money.amount, money.currency, entry.at) : null
  // Money that leaves lands in a wallet or, for a payment, in a category.
  const to = money.to_store ? names.store(money.to_store) : money.category ? names.category(money.category) : ''
  const from = money.from_store ? names.store(money.from_store) : ''
  if (sum && to) return [sum, { text: ` → ${to}` }]
  if (sum && from) return [sum, { text: ` · ${from}` }]
  return sum ? [sum] : entry.note ? [{ text: entry.note }] : []
}
