/** Commands: wallets, envelopes, the exchange terminal, overviews and history. */

import { emptyTxFilter } from '@/shared/lib/tx-filter'
import { mockListTxs, mockTxGraph } from './explore'
import { mockEnvelopeView, mockEnvelopes, mockWalletCatalog } from './catalog'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockStores } from './demo'
import { mockHistory, mockSummary } from './market'
import { mockExploreCtx, mockOverview, mockStore } from './tree'
import { mockSettings } from './vault-data'

/** Commands: wallets, envelopes and the exchange terminal. */
export const walletsCommands: MockCommands = {
  list_stores: (_call) => {
    return resolve(mockStores)
  },
  get_wallet_catalog: (_call) => {
    return resolve(mockWalletCatalog())
  },
  list_envelopes: ({ args }) => {
    const storeId = args?.storeId ? String(args.storeId) : null
    return resolve(mockEnvelopes.filter((entry) => !storeId || entry.store_id === storeId).map(mockEnvelopeView))
  },
  create_envelope: ({ body }) => {
    const draft = body as EnvelopeDraft
    const envelope: Envelope = {
      id: crypto.randomUUID(),
      store_id: draft.store_id,
      name: draft.name.trim(),
      asset: draft.asset,
      allocated: '0',
      target: draft.target,
      goal_id: draft.goal_id,
      created_at: new Date().toISOString(),
    }
    mockEnvelopes.push(envelope)
    return resolve(envelope)
  },
  update_envelope: ({ body }) => {
    const input = body as UpdateEnvelopeInput
    const envelope = mockEnvelopes.find((entry) => entry.id === input.id)
    if (!envelope) throw { code: 'MISSING', message: 'envelope' }
    Object.assign(envelope, { name: input.draft.name, target: input.draft.target, goal_id: input.draft.goal_id })
    return resolve(envelope)
  },
  fund_envelope: ({ body }) => {
    const input = body as FundEnvelopeInput
    const envelope = mockEnvelopes.find((entry) => entry.id === input.id)
    if (!envelope) throw { code: 'MISSING', message: 'envelope' }
    const view = mockEnvelopeView(envelope)
    const amount = Number(input.amount)
    if (amount > 0 && Number(view.free) < amount)
      throw { code: 'INSUFFICIENT_FUNDS', message: `only ${view.free} is free` }
    if (Number(envelope.allocated) + amount < 0) throw { code: 'INVALID_INPUT', message: 'the envelope holds less' }
    envelope.allocated = String(Number(envelope.allocated) + amount)
    return resolve(envelope)
  },
  move_between_envelopes: ({ body }) => {
    const input = body as MoveEnvelopeInput
    const from = mockEnvelopes.find((entry) => entry.id === input.from)
    const to = mockEnvelopes.find((entry) => entry.id === input.to)
    if (!from || !to) throw { code: 'MISSING', message: 'envelope' }
    from.allocated = String(Number(from.allocated) - Number(input.amount))
    to.allocated = String(Number(to.allocated) + Number(input.amount))
    return resolve({})
  },
  delete_envelope: ({ args }) => {
    const index = mockEnvelopes.findIndex((entry) => entry.id === String(args?.id))
    if (index >= 0) mockEnvelopes.splice(index, 1)
    return resolve({})
  },
  get_terminal_state: ({ args }) => {
    const store = mockStore(String(args?.storeId ?? ''))
    if (!store?.exchange) throw { code: 'PROVIDER_REQUIRED', message: 'choose the exchange first' }
    return resolve(store.exchange.terminal)
  },
  update_terminal_state: ({ body }) => {
    const input = body as UpdateTerminalStateInput
    const store = mockStore(input.store_id)
    if (!store?.exchange) throw { code: 'PROVIDER_REQUIRED', message: 'choose the exchange first' }
    const terminal = store.exchange.terminal
    const patch = input.patch
    if (patch.watchlist) terminal.watchlist = [...new Set(patch.watchlist.map((entry) => entry.toUpperCase()))]
    if (patch.active_symbol) terminal.active_symbol = patch.active_symbol.toUpperCase()
    if (patch.timeframe) terminal.timeframe = patch.timeframe
    if (patch.indicators) terminal.indicators = patch.indicators
    if (patch.layout) terminal.layout = patch.layout
    return resolve(terminal)
  },
  save_terminal_drawings: ({ body }) => {
    const input = body as SaveTerminalDrawingsInput
    const store = mockStore(input.store_id)
    if (!store?.exchange) throw { code: 'PROVIDER_REQUIRED', message: 'choose the exchange first' }
    const symbol = input.symbol.toUpperCase()
    if (!input.json.trim() || input.json.trim() === '[]') delete store.exchange.terminal.drawings[symbol]
    else store.exchange.terminal.drawings[symbol] = input.json
    return resolve({})
  },
  set_import_interval: ({ body }) => {
    const input = body as SetImportIntervalInput
    const store = mockStore(input.store_id)
    if (!store?.exchange) throw { code: 'PROVIDER_REQUIRED', message: 'choose the exchange first' }
    store.exchange.import_interval_sec = input.interval_sec
    return resolve({})
  },
}

/** Commands: lists, overviews and history of a wallet. */
export const overviewCommands: MockCommands = {
  list_transactions: ({ args }) => {
    // Tauri v2 converts primitive command args to camelCase, so the mock must
    // read the same names the real backend receives (data *fields* stay snake_case).
    return resolve(mockListTxs((args?.filter as TxFilter | null) ?? emptyTxFilter(), mockExploreCtx()))
  },
  get_tx_graph: ({ args }) => {
    return resolve(
      mockTxGraph(
        (args?.filter as TxFilter | null) ?? emptyTxFilter(),
        (args?.collapsed as string[] | null) ?? [],
        mockExploreCtx(),
      ),
    )
  },
  get_store_overview: ({ args }) => {
    return resolve(mockOverview(String(args?.storeId ?? ''), args?.includeChildren !== false))
  },
  get_store_history: (_call) => {
    return resolve(mockHistory)
  },
  get_store_pnl: (_call) => {
    return resolve({ total_base: '1234.56', fees_base: '4.5', legs: [] })
  },
  get_summary: (_call) => {
    return resolve(mockSummary)
  },
  get_history: (_call) => {
    return resolve(mockHistory)
  },
  get_realized_pnl: (_call) => {
    return resolve({ total_base: '1234.56', fees_base: '4.5', legs: [] })
  },
  get_settings: (_call) => {
    return resolve(mockSettings)
  },
}
