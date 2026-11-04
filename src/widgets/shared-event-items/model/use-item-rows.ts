import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useI18n } from 'vue-i18n'

import { ME, diffText, planFact, qtyText, splitCaption, sumDecimals } from '@/entities/shared-event'
import { formatList, formatMoney, formatQuantity } from '@/shared/lib/format'

import type { ItemRow } from '../shared-event-items.types'

/** The rows of the items table, worked out once per refresh. */
export function useItemRows(view: MaybeRefOrGetter<SharedEventView | undefined>) {
  const { t } = useI18n()

  const nameOf = (id: string) =>
    toValue(view)?.people.find((person) => person.id === id)?.name || (id === ME ? t('sharedEvents.me') : id)

  return computed<ItemRow[]>(() => {
    const current = toValue(view)
    if (!current) return []
    return current.event.items.flatMap((item) => {
      const itemView = current.items.find((entry) => entry.item_id === item.id)
      if (!itemView) return []
      const fact = planFact(item, itemView)
      const money = (value: string) =>
        // money: plain — the accessible name is text
        formatMoney(value, { currency: current.event.currency, decimals: sumDecimals(current) })
      const summary = [
        qtyText(fact, item.unit, formatQuantity, t('sharedEvents.items.qtyOf')),
        fact.diffMoney !== null ? diffText(fact.diffMoney, money) : fact.state === 'unplanned' ? money(fact.cost) : '',
      ]
        .filter(Boolean)
        .join(' · ')
      const caption = splitCaption(item.split, nameOf, formatQuantity)
      const buyers = [...new Set(item.purchases.map((purchase) => purchase.buyer))].map((id) =>
        id === ME ? t('sharedEvents.items.byMeShort') : nameOf(id),
      )
      return [
        {
          item,
          view: itemView,
          fact,
          caption:
            caption.mode === 'all'
              ? ''
              : caption.mode === 'units'
                ? t('sharedEvents.items.splitUnits', { parts: formatList(caption.parts) })
                : t('sharedEvents.items.splitSome', { names: formatList(caption.parts) }),
          summary,
          needsRate: current.purchases.some((purchase) => purchase.item_id === item.id && purchase.needs_rate),
          boughtBy: buyers.length ? t('sharedEvents.items.boughtBy', { names: formatList(buyers) }) : '',
        },
      ]
    })
  })
}
