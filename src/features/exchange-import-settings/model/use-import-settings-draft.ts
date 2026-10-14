import { reactive, ref, watch } from 'vue'

import { toIsoTimestamp } from '@/shared/lib/datetime'

import { IMPORT_ACCOUNTS, IMPORT_KINDS } from '../exchange-import-settings.constants'
import type { ImportRouteMode } from '../exchange-import-settings.types'

/** The import settings being edited: filled from the saved ones, read back as `ImportSettings`. */
export function useImportSettingsDraft(saved: () => ImportSettings | undefined) {
  const flags = reactive({ trades: true, transfers: true, earn: true, auto_confirm: false, auto_reconcile: true })
  const categories = reactive<Record<(typeof IMPORT_KINDS)[number], string>>({ trades: '', transfers: '', earn: '' })
  const routes = reactive<Record<string, { mode: ImportRouteMode; storeId: string }>>(
    Object.fromEntries(IMPORT_ACCOUNTS.map((account) => [account, { mode: 'default', storeId: '' }])),
  )
  const coins = ref<string[]>([])
  const since = ref('')
  const coinPick = ref<AssetRef | null>(null)

  watch(
    saved,
    (settings) => {
      if (!settings) return
      Object.assign(flags, {
        trades: settings.trades,
        transfers: settings.transfers,
        earn: settings.earn,
        auto_confirm: settings.auto_confirm,
        auto_reconcile: settings.auto_reconcile,
      })
      for (const kind of IMPORT_KINDS) categories[kind] = settings.categories[kind] ?? ''
      for (const account of IMPORT_ACCOUNTS) {
        const route = settings.accounts.find((entry) => entry.account === account)
        routes[account] = route
          ? { mode: route.store_id ? 'store' : 'skip', storeId: route.store_id ?? '' }
          : { mode: 'default', storeId: '' }
      }
      coins.value = [...settings.coins]
      since.value = settings.since?.slice(0, 10) ?? ''
    },
    { immediate: true },
  )

  // A coin picked in the search joins the list at once.
  watch(coinPick, (asset) => {
    if (!asset) return
    const symbol = asset.symbol.toUpperCase()
    if (!coins.value.includes(symbol)) coins.value = [...coins.value, symbol]
    coinPick.value = null
  })

  /** The settings as the form has them now, saved or not. */
  function currentSettings(): ImportSettings {
    return {
      ...flags,
      coins: coins.value,
      since: since.value ? toIsoTimestamp(since.value) : null,
      categories: {
        trades: categories.trades || null,
        transfers: categories.transfers || null,
        earn: categories.earn || null,
      },
      accounts: IMPORT_ACCOUNTS.filter((account) => routes[account].mode !== 'default').map((account) => ({
        account,
        store_id: routes[account].mode === 'store' ? routes[account].storeId : null,
      })),
    }
  }

  return { flags, categories, routes, coins, since, coinPick, currentSettings }
}
