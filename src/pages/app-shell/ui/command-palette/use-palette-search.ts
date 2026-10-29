import { type Ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { refDebounced } from '@vueuse/core'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { useReport } from '@/entities/report'
import { call } from '@/shared/api/ipc'
import { currentMonth } from '@/shared/lib/period'
import { entityKey, sameEntity } from '@/shared/lib/entity-link'
import { recentEntities } from '@/shared/lib/recent-entities'
import { useTxContext } from '@/shared/lib/tx-context'

/** Shown in this order; wallets have their own group in the palette. */
const KIND_ORDER: EntityKind[] = [
  'category',
  'goal',
  'budget',
  'contact',
  'asset',
  'automation',
  'workflow',
  'reminder',
  'alert',
  'scenario',
]
const TX_LIMIT = 20
const TX_MIN_CHARS = 2

export interface PaletteHit {
  id: string
  target: EntityRef
  label: string
  archived: boolean
}

export interface PaletteRecentHit extends PaletteHit {
  at?: string
}

/**
 * The palette's search over the vault: every entity by name (the index loads
 * once the palette opens), operations by text or amount once two letters are
 * typed, and the cards opened lately.
 */
export function usePaletteSearch(open: Ref<boolean>, search: Ref<string>) {
  const { t } = useI18n()
  const context = useTxContext()

  const index = useQuery({
    queryKey: ['search-index'],
    queryFn: () => call<SearchEntry[]>('get_search_index'),
    enabled: open,
    staleTime: 30_000,
  })

  /** The core leaves built-in categories and limits unnamed: they are named here, as on their cards. */
  function labelOf(entry: SearchEntry): string {
    if (entry.label) return entry.label
    if (entry.target.kind === 'category') return context.categoryName(entry.target.id)
    if (entry.target.kind === 'budget') {
      const subject = entry.subject
      const what = !subject
        ? t('palette.budgetTotal')
        : subject.kind === 'category'
          ? context.categoryName(subject.id)
          : context.storeName(subject.id)
      return `${t('entity.kind.budget')}: ${what}`
    }
    return t(`entity.kind.${entry.target.kind}`)
  }

  const hits = computed<PaletteHit[]>(() =>
    (index.data.value ?? []).map((entry) => ({
      id: `entity-${entityKey(entry.target)}`,
      target: entry.target,
      label: labelOf(entry),
      archived: entry.archived,
    })),
  )
  const groups = computed(() =>
    KIND_ORDER.map((kind) => ({ kind, items: hits.value.filter((hit) => hit.target.kind === kind) })).filter(
      (group) => group.items.length,
    ),
  )

  const recent = computed<PaletteRecentHit[]>(() =>
    recentEntities.value.map(({ target, at, title }) => {
      const known = hits.value.find((hit) => sameEntity(hit.target, target))
      const label = title ?? known?.label ?? t(`entity.kind.${target.kind}`)
      return { id: `recent-${entityKey(target)}`, target, label, archived: known?.archived ?? false, at }
    }),
  )

  const typed = refDebounced(
    computed(() => search.value.trim()),
    250,
  )
  /** Built-in categories are matched by their translated name, which only the frontend knows. */
  const categories = computed(() => {
    const needle = typed.value.toLowerCase()
    return hits.value
      .filter((hit) => hit.target.kind === 'category' && hit.label.toLowerCase().includes(needle))
      .map((hit) => hit.target.id)
  })
  const transactions = useQuery({
    queryKey: computed(() => ['search-transactions', typed.value]),
    queryFn: () =>
      call<Transaction[]>('search_transactions', { query: typed.value, categories: categories.value, limit: TX_LIMIT }),
    enabled: computed(() => open.value && typed.value.length >= TX_MIN_CHARS),
    placeholderData: keepPreviousData,
  })
  const found = computed(() => (search.value.trim().length >= TX_MIN_CHARS ? (transactions.data.value ?? []) : []))

  /** The month's spending per category: `Groceries · 36 $ in Oct` (Paths 2 · 8). */
  const month = currentMonth()
  const report = useReport(
    'categories',
    {
      scope: { store_ids: [], include_children: true },
      from: month,
      to: month,
      year: Number(month.slice(0, 4)),
      currency: null,
      real_terms: false,
    },
    { enabled: () => open.value && Boolean(search.value.trim()) },
  )
  const monthTotals = computed(() => {
    const data = report.data.value
    const view = data && 'categories' in data ? data.categories : null
    const totals = new Map<string, string>()
    for (const row of view?.groups.flatMap((block) => block.rows) ?? []) totals.set(row.key, row.total)
    return { totals, currency: view?.meta.currency ?? '', month }
  })
  /** The first category the query names: its actions follow the results. */
  const category = computed(() => {
    const needle = search.value.trim().toLowerCase()
    if (!needle) return null
    return hits.value.find((hit) => hit.target.kind === 'category' && hit.label.toLowerCase().includes(needle)) ?? null
  })

  return { hits, groups, recent, transactions: found, monthTotals, category }
}
