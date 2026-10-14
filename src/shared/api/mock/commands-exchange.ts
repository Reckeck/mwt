/** Commands: the exchange key, Bybit accounts and the import staging. */

import { mockImportSettings } from './catalog'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockStores, newStore } from './demo'
import {
  mockExecutions,
  mockImportRuns,
  mockImportSettingsByStore,
  mockKeyStatus,
  mockPendingImports,
  mockPendingStore,
} from './exchange-data'

/** Commands: exchange (read-only prefill). */
export const exchangeReadOnlyCommands: MockCommands = {
  inspect_tx: (_call) => {
    // Offline in the browser mock: no suggestions, form left untouched.
    return resolve({ fetched: false, info: null, hints: [], notice: 'unavailable' })
  },
  exchange_key_status: (_call) => {
    return resolve(mockKeyStatus.value)
  },
  set_exchange_key: ({ body }) => {
    const input = body as SetExchangeKeyInput
    const apiKey = input.api_key
    mockKeyStatus.value = {
      ...mockKeyStatus.value,
      configured: true,
      hint: apiKey.length <= 8 ? '*'.repeat(apiKey.length) : `${apiKey.slice(0, 4)}…${apiKey.slice(-4)}`,
    }
    return resolve(mockKeyStatus.value)
  },
  set_exchange_targets: ({ body }) => {
    const input = body as SetExchangeTargetsInput
    mockKeyStatus.value = {
      ...mockKeyStatus.value,
      spot_store_id: input.spot_store_id,
      futures_store_id: input.futures_store_id,
    }
    return resolve(mockKeyStatus.value)
  },
  clear_exchange_key: (_call) => {
    mockKeyStatus.value = { configured: false, hint: null, spot_store_id: null, futures_store_id: null }
    return resolve(mockKeyStatus.value)
  },
  bybit_executions: (_call) => {
    return resolve(
      mockKeyStatus.value.configured
        ? { executions: mockExecutions, notice: null }
        : { executions: [], notice: 'no_key' },
    )
  },
}

/** Commands: Bybit sub-accounts and balances. */
export const exchangeAccountsCommands: MockCommands = {
  create_bybit_accounts: ({ body }) => {
    const input = body as CreateBybitAccountsInput
    const root = mockStores.find((store) => store.id === input.store_id)
    if (!root) return Promise.reject({ code: 'MISSING', message: 'store' })
    const wanted: Array<[SubType, MarketKind, string]> = [
      ['funding', 'funding', input.funding_name],
      ['unified', 'unified', input.unified_name],
      ['earn', 'earn', input.earn_name],
    ]
    // The mock's Bybit holds coins in the unified account only.
    const used = (market: MarketKind) => !input.detect || market === 'unified'
    const ids = wanted.map(([subType, market, name]) => {
      const known = mockStores.find(
        (store) =>
          store.parent_id === root.id &&
          typeof store.binding === 'object' &&
          'market' in store.binding &&
          store.binding.market === market,
      )
      if (known) return known.id
      if (!used(market)) return ''
      const created: Store = {
        ...newStore(name, 'exchange', root.color, root.id),
        sub_type: subType,
        binding: { market },
        exchange: null,
      }
      mockStores.push(created)
      return created.id
    })
    return resolve(ids.filter(Boolean))
  },
  bybit_balances: (_call) => {
    return resolve(
      mockKeyStatus.value.configured
        ? ([
            { account: 'FUND', store_id: null, rows: [{ coin: 'USDT', exchange: '250', ledger: '0', diff: '250' }] },
            {
              account: 'UNIFIED',
              store_id: null,
              rows: [{ coin: 'BTC', exchange: '0.02', ledger: '0.02', diff: '0' }],
            },
            { account: 'EARN', store_id: null, rows: [] },
          ] satisfies ExchangeAccountBalance[])
        : [],
    )
  },
}

/** Commands: bybit import staging (D2.4). */
export const bybitImportStagingCommands: MockCommands = {
  import_bybit_history: ({ body }) => {
    if (!mockKeyStatus.value.configured) return resolve({ staged: 0, notice: 'no_key', run: null })
    // Records belong to the wallet synced, as the backend stages them.
    mockPendingStore.value = mockPendingImports().map((entry) => ({ ...entry, store_id: String(body?.store_id ?? '') }))
    const run: ImportRun = {
      id: crypto.randomUUID(),
      store_id: String(body?.store_id ?? ''),
      at: new Date().toISOString(),
      source: 'manual',
      fetched: mockPendingImports().length,
      staged: mockPendingImports().length,
      known: 0,
      filtered: 0,
      confirmed: 0,
      reconcile_tx_id: null,
      error: null,
    }
    mockImportRuns.unshift(run)
    return resolve({ staged: run.staged, notice: null, run })
  },
  preview_bybit_import: (_call) => {
    if (!mockKeyStatus.value.configured) return resolve({ preview: null, notice: 'no_key' })
    const items = mockPendingImports()
    return resolve({
      preview: { fetched: items.length, known: 0, filtered: 0, new: items.length, items },
      notice: null,
    })
  },
  list_import_runs: ({ args }) => {
    return resolve(mockImportRuns.filter((run) => run.store_id === args?.storeId))
  },
  get_import_settings: ({ args }) => {
    return resolve(mockImportSettingsByStore.get(String(args?.storeId)) ?? mockImportSettings())
  },
  set_import_settings: ({ body }) => {
    const input = body as unknown as SetImportSettingsInput
    mockImportSettingsByStore.set(input.store_id, input.settings)
    return resolve(input.settings)
  },
  list_imported_transactions: (_call) => {
    return resolve({ total: 0, items: [] })
  },
  list_pending_imports: (_call) => {
    return resolve({ imports: mockPendingImports() })
  },
  confirm_pending_imports: ({ body }) => {
    const input = body as ConfirmPendingImportsInput
    const ids = input.exec_ids.length ? input.exec_ids : mockPendingImports().map((entry) => entry.exec_id)
    const staged = mockPendingImports()
    mockPendingStore.value = staged.filter((entry) => !ids.includes(entry.exec_id))
    return resolve({ confirmed: staged.length - mockPendingStore.value.length })
  },
  discard_pending_imports: ({ body }) => {
    const input = body as DiscardPendingImportsInput
    const ids = input.exec_ids.length ? input.exec_ids : mockPendingImports().map((entry) => entry.exec_id)
    const staged = mockPendingImports()
    mockPendingStore.value = staged.filter((entry) => !ids.includes(entry.exec_id))
    return resolve({ discarded: staged.length - mockPendingStore.value.length })
  },
}
