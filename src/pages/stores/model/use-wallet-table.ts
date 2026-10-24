import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { useConversions, useFavoriteCurrencies } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { formatAsOf } from '@/shared/lib/format'
import { withCryptoShown, withoutLoyalty } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

import type { WalletMode, WalletSort } from '../stores.types'

const SORTS: WalletSort[] = ['value', 'name']
const MODES: WalletMode[] = ['base', 'second', 'assets']

/**
 * The wallets table (Wallets-1 · 1, Currency-1 · 2): the roots, how they are
 * ordered, which currency the total and the last column are in. The order, the
 * mode and «show archived» live in the URL, so «back» returns to the same view.
 */
export function useWalletTable() {
  const { t } = useI18n()
  const route = useRoute()
  const router = useRouter()
  const settings = useSettingsStore()
  const favorites = useFavoriteCurrencies()
  const conversions = useConversions()
  const tree = useStoreTree()
  const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary') })

  const queryValue = <T extends string>(key: string, list: T[], fallback: T) =>
    computed<T>({
      get: () => (list.includes(route.query[key] as T) ? (route.query[key] as T) : fallback),
      set: (value) => void router.replace({ query: { ...route.query, [key]: value === fallback ? undefined : value } }),
    })
  const sort = queryValue('sort', SORTS, 'value')
  const mode = queryValue('view', MODES, 'base')
  const showArchived = computed({
    get: () => route.query.archived === '1',
    set: (value: boolean) => void router.replace({ query: { ...route.query, archived: value ? '1' : undefined } }),
  })

  const base = computed(() => settings.baseCurrency.toLowerCase())

  const allRoots = computed(() => withoutLoyalty(tree.data.value ?? []))
  /** Hidden crypto wallets leave the list, not the total. */
  const roots = computed(() => withCryptoShown(allRoots.value, settings.cryptoEnabled))
  const archivedCount = computed(() => roots.value.filter((node) => node.store.archived).length)
  const total = computed(() =>
    allRoots.value.filter((node) => !node.store.archived).reduce((sum, node) => sum + Number(node.subtree_base), 0),
  )
  /** «₽ 3 295 090 · 35 147 $ · 87 %»: what the wallets hold, merged from the rows' balances, biggest first. */
  const assets = computed(() => {
    const merged = new Map<string, AssetBalance>()
    for (const balance of allRoots.value.filter((node) => !node.store.archived).flatMap((node) => node.balances)) {
      const key = balance.asset.symbol.toUpperCase()
      const seen = merged.get(key)
      merged.set(
        key,
        seen
          ? {
              ...seen,
              quantity: String(Number(seen.quantity) + Number(balance.quantity)),
              value_base: String(Number(seen.value_base) + Number(balance.value_base)),
            }
          : balance,
      )
    }
    const list = [...merged.values()].filter((row) => Number(row.value_base) > 0)
    const sum = list.reduce((acc, row) => acc + Number(row.value_base), 0)
    return list
      .map((row) => ({ ...row, share: sum ? Number(row.value_base) / sum : 0 }))
      .sort((left, right) => Number(right.value_base) - Number(left.value_base))
  })
  /** «₽»: the budget's currency, or else a favourite, else the biggest fiat held. */
  const second = computed(() => {
    const budget = (settings.vault?.budget_currency || '').toLowerCase()
    if (budget && budget !== base.value) return budget
    const favorite = favorites.codes.value.find((code) => code !== base.value)
    if (favorite) return favorite
    // Nothing set: the biggest fiat the wallets hold besides the base.
    const held = assets.value.find((row) => row.asset.chain === 'fiat' && row.asset.symbol.toLowerCase() !== base.value)
    return held?.asset.symbol.toLowerCase() ?? ''
  })
  const currency = computed(() => (mode.value === 'second' && second.value ? second.value : base.value))
  /** A base amount in the chosen currency; `null` while the rate is missing. */
  const shown = (amount: number) => conversions.convertTo(amount, base.value, currency.value)
  const rows = computed(() => {
    const list = roots.value.filter((node) => showArchived.value || !node.store.archived)
    if (sort.value === 'name') return [...list].sort((left, right) => left.store.name.localeCompare(right.store.name))
    return [...list].sort((left, right) => Number(right.subtree_base) - Number(left.subtree_base))
  })

  /** «курсы на сегодня 14:20», or the date when they are older. */
  const rates = computed(() => {
    const at = summary.data.value?.prices_as_of ?? summary.data.value?.base_fx_as_of
    if (!at) return ''
    const today = new Date(at).toDateString() === new Date().toDateString()
    return t(today ? 'stores.ratesToday' : 'stores.ratesAt', { time: formatAsOf(at) })
  })

  return {
    tree,
    sort,
    mode,
    showArchived,
    base,
    second,
    currency,
    shown,
    roots,
    rows,
    archivedCount,
    total,
    assets,
    rates,
  }
}
