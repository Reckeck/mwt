import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { currencySymbol, formatDate, formatQuantity, pluralForm } from '@/shared/lib/format'
import { composeSums, type SumPart } from '@/shared/lib/money-parts'
import { flattenStoreTree } from '@/shared/lib/store-tree'

import type { ProgramRow } from '../bonus-programs.types'

const MONTH_MS = 30 * 86_400_000

/** The programmes with their balances, worded for the table: what they are, how much, what it is worth. */
export function useProgramRows() {
  const { t, locale } = useI18n()
  const tree = useStoreTree()
  const report = useQuery({
    queryKey: ['cashback', 'year'],
    placeholderData: keepPreviousData,
    queryFn: () => call<CashbackReport>('get_cashback_report', { period: 'year' }),
  })

  const nodes = computed(() => flattenStoreTree(tree.data.value ?? []))
  const names = computed(() => new Map(nodes.value.map((node) => [node.store.id, node.store.name])))

  /** `1,240 points`, `1 point`. */
  function count(unit: LoyaltyUnit, value: string): string {
    const shown = formatQuantity(value, 0)
    return t(`loyaltyUnit.count.${unit}.${pluralForm(Number(value), locale.value)}`, { count: shown })
  }

  /** `Tinkoff · points = ₽` when a point is a unit of the currency, else `· 1 coupon = 2.50 $`. */
  function subtitle(store: Store): SumPart[] {
    const info = store.loyalty
    if (!info) return [{ text: t('loyalty.setup') }]
    const wallet = names.value.get(info.source_store_ids[0] ?? '') ?? ''
    const { mark, parts } = composeSums()
    const rate =
      Number(info.point_value) === 1
        ? `${t(`loyaltyUnit.${info.unit}`).toLowerCase()} = ${currencySymbol(info.currency)}`
        : `${t(`loyaltyUnit.one.${info.unit}`)} = ${mark({ amount: info.point_value, currency: info.currency })}`
    return parts([wallet, rate].filter(Boolean).join(' · '))
  }

  const rows = computed<ProgramRow[]>(() =>
    nodes.value
      .filter((node) => node.store.kind === 'loyalty' && !node.store.archived)
      .map(({ store }) => {
        const balance = report.data.value?.programmes.find((row) => row.store_id === store.id) ?? null
        const unit = store.loyalty?.unit ?? 'points'
        const burns = balance?.expires_at ?? null
        return {
          store,
          subtitle: subtitle(store),
          points: count(unit, balance?.points ?? '0'),
          valueBase: balance?.value_base ?? '0',
          expiring: burns
            ? t('bonuses.burns', { points: count(unit, balance?.expiring_points ?? '0'), date: formatDate(burns) })
            : '',
          soon: !!burns && new Date(burns).getTime() - Date.now() < MONTH_MS,
        }
      })
      .sort((left, right) => Number(right.valueBase) - Number(left.valueBase)),
  )
  const total = computed(() => rows.value.reduce((sum, row) => sum + Number(row.valueBase), 0))
  const baseCurrency = computed(() => report.data.value?.base_currency ?? 'usd')

  return { tree, rows, total, baseCurrency }
}
