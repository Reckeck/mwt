/** Commands: wallets, envelopes, the exchange terminal, overviews and history. */

import { emptyTxFilter } from '@/shared/lib/tx-filter'
import { txLegs, txMeta } from '@/shared/lib/transactions'
import { mockListTxs, mockTxGraph } from './explore'
import { mockEnvelopeView, mockEnvelopes, mockWalletCatalog } from './catalog'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockFilterTxs, mockStores } from './demo'
import { mockHistory, mockNetWorth, mockSummary } from './market'
import { mockInvestments } from './investments'
import { mockAncestors, mockExploreCtx, mockOverview, mockOwnPositions, mockStore } from './tree'
import { goals } from './planning/state'
import { mockAppSettings, mockSettings } from './vault-data'

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
  fund_goal: ({ body }) => {
    const input = body as FundGoalInput
    const goal = goals.find((entry) => entry.id === input.goal_id)
    if (!goal) throw { code: 'MISSING', message: `goal ${input.goal_id}` }
    const position = mockOwnPositions(input.store_id).find(
      (entry) => entry.asset.symbol.toLowerCase() === goal.currency.toLowerCase(),
    )
    if (!position) throw { code: 'INVALID_INPUT', message: `the wallet holds no ${goal.currency}` }
    let envelope = mockEnvelopes.find((entry) => entry.store_id === input.store_id && entry.goal_id === goal.id)
    if (!envelope) {
      envelope = {
        id: crypto.randomUUID(),
        store_id: input.store_id,
        name: goal.name,
        asset: position.asset,
        allocated: '0',
        target: null,
        goal_id: goal.id,
        created_at: new Date().toISOString(),
      }
      mockEnvelopes.push(envelope)
    }
    const take = Math.max(0, Math.min(Number(input.amount), Number(mockEnvelopeView(envelope).free)))
    envelope.allocated = String(Number(envelope.allocated) + take)
    return resolve(String(take))
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
  list_transactions_page: ({ args }) => {
    const all = mockListTxs((args?.filter as TxFilter | null) ?? emptyTxFilter(), mockExploreCtx())
    const offset = Number(args?.offset ?? 0)
    const limit = Number(args?.limit ?? all.length)
    return resolve({ items: all.slice(offset, offset + limit), total: all.length } satisfies TxPage)
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
  get_investments: ({ args }) => resolve(mockInvestments((args?.storeIds as string[] | undefined) ?? [])),
  // Like `asset_holdings` in Rust, matched by ticker only.
  get_asset_holdings: ({ args }) => {
    const asset = args?.asset as AssetRef
    const rows: AssetHolding[] = mockStores.flatMap((store) =>
      mockOwnPositions(store.id)
        .filter((position) => position.asset.symbol.toUpperCase() === asset.symbol.toUpperCase())
        .map((position) => ({
          store_id: store.id,
          store_name: store.name,
          path: mockAncestors(store.id).map((parent) => parent.name),
          root_id: mockAncestors(store.id)[0]?.id ?? store.id,
          archived: store.archived,
          asset: position.asset,
          quantity: position.quantity,
          avg_cost: position.avg_cost,
          price_base: position.price_base,
          value_base: position.value_base,
          unrealized_base: position.unrealized_base,
          price_info: position.price_info,
          peg_valued: position.price_info.origin === 'peg',
        })),
    )
    rows.sort((left, right) => Number(right.value_base) - Number(left.value_base))
    const sum = (pick: (row: AssetHolding) => string) =>
      String(rows.reduce((total, row) => total + Number(pick(row)), 0))
    return resolve({
      asset,
      base_currency: mockAppSettings.base_currency,
      total_quantity: sum((row) => row.quantity),
      total_value_base: sum((row) => row.value_base),
      rows,
    } satisfies AssetHoldings)
  },
  // Like `store_months` in Rust: principal legs on the wallet, valued at their recorded price.
  get_store_months: ({ args }) => {
    const storeId = String(args?.storeId ?? '')
    const count = Number(args?.months ?? 6)
    const now = new Date()
    const rows: StoreMonthRow[] = Array.from({ length: count }, (_unused, index) => {
      const date = new Date(now.getFullYear(), now.getMonth() - (count - 1 - index), 1)
      return {
        month: `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`,
        inflow: '0',
        outflow: '0',
        tx_count: 0,
      }
    })
    for (const tx of mockFilterTxs([storeId])) {
      const row = rows.find((entry) => txMeta(tx).timestamp.startsWith(entry.month))
      if (!row) continue
      row.tx_count += 1
      for (const leg of txLegs(tx).filter((entry) => entry.store_id === storeId && entry.role === 'principal')) {
        const value = Math.abs(Number(leg.amount)) * Number(leg.price_usd ?? 1)
        if (Number(leg.amount) < 0) row.outflow = String(Number(row.outflow) + value)
        else row.inflow = String(Number(row.inflow) + value)
      }
    }
    return resolve(rows)
  },
  get_store_history: (_call) => {
    return resolve(mockHistory)
  },
  get_store_pnl: (_call) => {
    return resolve({ total_base: '1234.56', fees_base: '4.5', legs: [] })
  },
  get_summary: (_call) => {
    // Like the core: the summary's net worth is the same figure `get_net_worth` gives.
    const worth = mockNetWorth()
    return resolve({
      ...mockSummary,
      net_worth_base: worth.net,
      liabilities_base: String(Number(worth.liabilities) + Number(worth.payables)),
    })
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
