import { useI18n } from 'vue-i18n'

import { formatDay } from '@/shared/lib/format'
import { useTxContext } from '@/shared/lib/tx-context'

/** One set filter as a chip: its short name and what takes it off. */
export interface FilterChip {
  key: string
  label: string
  /** Merged into the filter to clear just this one. */
  clear: Partial<TxFilter>
}

/** Chips a computer shows in the row itself: they have no field there (a report or a link set them). */
export const ROW_CHIP_KEYS = ['kinds', 'fee', 'realized', 'flow']

/**
 * Every set filter but the search, in the order of the «Фильтры» screen: the phone's line under the
 * search, the computer's row chips, the «Под фильтр ничего не попало» sentence. A wallet fixed by the
 * page (`fixedScope`) is not a filter the user set.
 */
export function useFilterChips(): (filter: TxFilter, fixedScope?: boolean) => FilterChip[] {
  const { t } = useI18n()
  const context = useTxContext()
  return (filter, fixedScope = false) => {
    const chips: FilterChip[] = []
    const add = (key: string, on: unknown, label: () => string, clear: Partial<TxFilter>) => {
      if (on) chips.push({ key, label: label(), clear })
    }
    const stores = filter.scope.store_ids
    add(
      'scope',
      !fixedScope && stores.length,
      () => (stores.length === 1 ? context.storeName(stores[0]) : t('filter.storesCount', { n: stores.length })),
      { scope: { store_ids: [], include_children: true } },
    )
    add(
      'kinds',
      filter.kinds.length,
      () =>
        filter.kinds.length === 1
          ? t(`tx.kind.${filter.kinds[0]}`)
          : t('filter.kindsCount', { n: filter.kinds.length }),
      { kinds: [] },
    )
    add('period', filter.from || filter.to, () => periodLabel(filter), { from: null, to: null })
    add('category', filter.category !== null, () => context.categoryName(filter.category ?? ''), { category: null })
    add('contact', filter.contact_id, () => context.contactName(filter.contact_id ?? ''), { contact_id: null })
    add(
      'amount',
      filter.amount_min || filter.amount_max,
      () => `${t('filter.amount')} ${filter.amount_min ?? ''}–${filter.amount_max ?? ''}`,
      { amount_min: null, amount_max: null },
    )
    add('flow', filter.flow_id, () => t('filter.flow'), { flow_id: null })
    add('fee', filter.with_fee, () => t('filter.withFee'), { with_fee: false })
    add('realized', filter.realized, () => t('filter.realized'), { realized: false })
    add('auto', filter.auto_only, () => t('filter.chipShort.auto'), { auto_only: false })
    add('debt', filter.debt_only, () => t('filter.chipShort.debt'), { debt_only: false })
    add('files', filter.with_attachments, () => t('filter.chipShort.files'), { with_attachments: false })
    add('internal', filter.hide_internal, () => t('filter.chipShort.internal'), { hide_internal: false })
    add('unpriced', filter.unpriced, () => t('filter.unpriced'), { unpriced: false })
    return chips
  }

  function periodLabel(filter: TxFilter): string {
    if (filter.from && filter.to) return `${formatDay(filter.from)} – ${formatDay(filter.to)}`
    return filter.from ? `${t('filter.from')} ${formatDay(filter.from)}` : `${t('filter.to')} ${formatDay(filter.to)}`
  }
}
