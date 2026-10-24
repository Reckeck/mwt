import type { WizardFrequency, WizardStarter } from '../automation-edit.constants'

/** The schedule «Как часто» and its day stand for; `null` while the day is not a day. */
export function wizardSchedule(frequency: WizardFrequency, day: string, weekday: string): Schedule | null {
  if (frequency === 'daily') return { every_n_days: { days: 1 } }
  if (frequency === 'weekly') return { weekly: { weekday: Number(weekday) } }
  const value = Number(day)
  return Number.isInteger(value) && value >= 1 && value <= 31 ? { monthly: { day: value } } : null
}

type Translate = (key: string, params?: Record<string, unknown>) => string

const sideStore = (side: AccountRef | null | undefined) => (side && 'store' in side ? side.store : null)

/**
 * A name the wizard gives, since the frame asks none: «Перевод на Bybit»,
 * «Оплата · Тинькофф», «Обмен · Bybit». It can be changed in the card later.
 */
export function wizardName(input: TransactionInput, storeName: (id: string) => string, t: Translate): string {
  if (input.type === 'transfer') {
    const to = sideStore(input.to)
    return to ? t('automationWizard.name.transferTo', { name: storeName(to) }) : t('tx.kindTab.transfer')
  }
  const store = 'store_id' in input ? input.store_id : input.type === 'convert' ? sideStore(input.from.account) : null
  const kind = t(`tx.kindTab.${input.type}`)
  return store ? `${kind} · ${storeName(store)}` : kind
}

/** What a ready-made start the wizard fills itself sets (Auto-1 · 6). */
export interface StarterPlan {
  kind: TxKind
  frequency: WizardFrequency
  day: string
  weekday: string
  workdayShift: WorkdayShift
  asks: boolean
}

/** «Покупка раз в неделю»: on Friday it asks, then exchanges. «Платёж в рабочий день»: the 10th, a day off moves it to the Friday before. */
export const STARTER_PLANS: Partial<Record<WizardStarter, StarterPlan>> = {
  weekly_buy: { kind: 'convert', frequency: 'weekly', day: '1', weekday: '4', workdayShift: 'none', asks: true },
  workday_payment: {
    kind: 'payment',
    frequency: 'monthly',
    day: '10',
    weekday: '0',
    workdayShift: 'back',
    asks: false,
  },
}
