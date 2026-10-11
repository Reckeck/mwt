/// Central typed bridge to the Rust backend.
///
/// Inside Tauri uses `@tauri-apps/api/core`; in a plain browser (hot `pnpm dev`
/// without the desktop shell) falls back to a small in-memory mock so the UI
/// can be iterated on.

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { txContactIds, txId, txLegs, txMeta } from '@/shared/lib/transactions'
import { emptyTxFilter } from '@/shared/lib/tx-filter'

import {
  mockCategories,
  mockContactBalances,
  mockContacts,
  mockKindsFor,
  mockRules,
  mockTransactionFrom,
  mockTurnover,
} from './ipc-mock-ledger'
import { mockCalendar, mockReport } from './ipc-mock-analytics'
import { mockListTxs, mockTxGraph } from './ipc-mock-explore'
import { mockPlanning, seedUtilityDemo } from './ipc-mock-planning'

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export interface AppError {
  code: string
  message: string
}

export function parseError(raw: unknown): AppError {
  // `call` already throws the parsed shape (and the mock throws it directly).
  if (raw && typeof raw === 'object' && 'code' in raw && 'message' in raw) {
    const { code, message } = raw as Record<string, unknown>
    if (typeof code === 'string' && typeof message === 'string') return { code, message }
  }
  const text = typeof raw === 'string' ? raw : String(raw ?? 'Unknown error')
  const sep = text.indexOf(': ')
  if (sep > 0 && text.slice(0, sep) === text.slice(0, sep).toUpperCase()) {
    return { code: text.slice(0, sep), message: text.slice(sep + 2) }
  }
  return { code: 'UNKNOWN', message: text }
}

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) return mock(cmd, args) as Promise<T>
  try {
    return await invoke<T>(cmd, args)
  } catch (err) {
    throw parseError(err)
  }
}

export async function onEvent<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  // Quietly no-op in browser dev mode.
  if (!isTauri) return () => {}
  return listen<T>(event, (received) => handler(received.payload))
}

// --- browser dev mock --------------------------------------------------------

function mock(cmd: string, rawArgs?: Record<string, unknown>): Promise<unknown> {
  seedMock()
  // Like Tauri, which sends arguments as JSON: detaches Vue proxies the mock would otherwise store.
  const args = rawArgs ? (JSON.parse(JSON.stringify(rawArgs)) as Record<string, unknown>) : rawArgs
  const body = args?.input as Record<string, unknown> | undefined
  switch (cmd) {
    case 'vault_status':
      return resolve(mockStatus)
    case 'unlock_vault':
      if (body?.password === 'secret') {
        mockStatus = { path: String(body?.path ?? mockStatus.path), exists: true, unlocked: true, info: mockInfo }
        return resolve(mockInfo)
      }
      throw { code: 'SECRET', message: 'Wrong password' }
    case 'create_vault':
      mockStatus = { path: String(body?.path ?? mockStatus.path), exists: true, unlocked: true, info: mockInfo }
      return resolve(mockInfo)
    case 'change_password':
      if (body?.current_password !== 'secret') throw { code: 'SECRET', message: 'Wrong password' }
      return resolve(undefined)
    case 'lock_vault_cmd':
      mockStatus = { ...mockStatus, unlocked: false, info: null }
      return resolve({})
    case 'pick_vault_file':
      // No native dialog in the browser mock; report a plausible path so the
      // create/open flows stay observable during `pnpm dev`.
      return resolve(args?.save ? '/tmp/mock/new-vault.wallet' : '/tmp/mock/vault.wallet')
    case 'forget_recent_vault':
      return resolve({})
    case 'list_stores':
      return resolve(mockStores)
    case 'get_wallet_catalog':
      return resolve(mockWalletCatalog())
    case 'list_envelopes': {
      const storeId = args?.storeId ? String(args.storeId) : null
      return resolve(mockEnvelopes.filter((entry) => !storeId || entry.store_id === storeId).map(mockEnvelopeView))
    }
    case 'create_envelope': {
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
    }
    case 'update_envelope': {
      const input = body as UpdateEnvelopeInput
      const envelope = mockEnvelopes.find((entry) => entry.id === input.id)
      if (!envelope) throw { code: 'MISSING', message: 'envelope' }
      Object.assign(envelope, { name: input.draft.name, target: input.draft.target, goal_id: input.draft.goal_id })
      return resolve(envelope)
    }
    case 'fund_envelope': {
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
    }
    case 'move_between_envelopes': {
      const input = body as MoveEnvelopeInput
      const from = mockEnvelopes.find((entry) => entry.id === input.from)
      const to = mockEnvelopes.find((entry) => entry.id === input.to)
      if (!from || !to) throw { code: 'MISSING', message: 'envelope' }
      from.allocated = String(Number(from.allocated) - Number(input.amount))
      to.allocated = String(Number(to.allocated) + Number(input.amount))
      return resolve({})
    }
    case 'delete_envelope': {
      const index = mockEnvelopes.findIndex((entry) => entry.id === String(args?.id))
      if (index >= 0) mockEnvelopes.splice(index, 1)
      return resolve({})
    }
    case 'get_terminal_state': {
      const store = mockStore(String(args?.storeId ?? ''))
      if (!store?.exchange) throw { code: 'PROVIDER_REQUIRED', message: 'choose the exchange first' }
      return resolve(store.exchange.terminal)
    }
    case 'update_terminal_state': {
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
    }
    case 'save_terminal_drawings': {
      const input = body as SaveTerminalDrawingsInput
      const store = mockStore(input.store_id)
      if (!store?.exchange) throw { code: 'PROVIDER_REQUIRED', message: 'choose the exchange first' }
      const symbol = input.symbol.toUpperCase()
      if (!input.json.trim() || input.json.trim() === '[]') delete store.exchange.terminal.drawings[symbol]
      else store.exchange.terminal.drawings[symbol] = input.json
      return resolve({})
    }
    case 'set_import_interval': {
      const input = body as SetImportIntervalInput
      const store = mockStore(input.store_id)
      if (!store?.exchange) throw { code: 'PROVIDER_REQUIRED', message: 'choose the exchange first' }
      store.exchange.import_interval_sec = input.interval_sec
      return resolve({})
    }
    case 'get_store_tree':
      return resolve(mockTree())
    case 'store_move_targets':
      return resolve(mockMoveTargets(String(args?.id ?? '')))
    case 'move_store': {
      const input = body as MoveStoreInput
      const store = mockStore(input.id)
      if (!store) throw { code: 'MISSING', message: 'store' }
      const failure = mockPlacementError(input.id, input.parent_id, store.name)
      if (failure) throw failure
      store.parent_id = input.parent_id
      return resolve(store)
    }
    case 'create_store': {
      const input = body as CreateStoreInput
      const failure = mockPlacementError(null, input.parent_id ?? null, input.name)
      if (failure) throw failure
      const store = {
        id: crypto.randomUUID(),
        name: input.name,
        kind: input.kind,
        color: input.color,
        note: input.note,
        archived: false,
        parent_id: input.parent_id ?? null,
        contact_id: input.contact_id ?? null,
        liability: input.liability ?? null,
        sub_type: input.sub_type ?? null,
        binding: input.binding ?? (input.sub_type ? mockMarketBinding(input.sub_type) : 'any'),
        // Like the backend: the sub-account type's purpose, else the parent's, else the kind's.
        purpose: input.purpose ?? mockDefaultPurpose(input),
        custody: input.kind === 'crypto' && !input.parent_id ? (input.custody ?? 'hot') : null,
        property: input.property ?? null,
        secured_by: input.secured_by ?? null,
        exchange: input.provider
          ? { provider: input.provider, import_interval_sec: 900, terminal: mockTerminal() }
          : null,
      }
      mockStores.push(store)
      // Like the backend: the opening balance is one top-up in a service category.
      if (input.opening && Number(input.opening.amount) > 0) {
        const { quote: _quote, ...asset } = input.opening.asset
        mockPush(
          mockTransactionFrom(
            {
              type: 'topup',
              common: {
                timestamp: new Date().toISOString(),
                comment: 'Opening balance',
                categories: ['opening_balance'],
                flow_id: null,
                contact_id: null,
                price_usd: null,
                fx_override: null,
                price: null,
                price_currency: null,
              },
              store_id: store.id,
              asset,
              amount: input.opening.amount,
              fee: null,
              from: null,
              details: { cash_breakdown: null, unit_price: null, network: null, tx_hash: null },
            },
            priceOf,
          ),
        )
      }
      return resolve(store)
    }
    case 'update_store': {
      const input = body as UpdateStoreInput
      const store = mockStore(input.id)
      if (!store) throw { code: 'MISSING', message: 'store' }
      const failure = mockPlacementError(input.id, store.parent_id, input.name)
      if (failure?.code === 'STORE_NAME_TAKEN') throw failure
      Object.assign(store, {
        name: input.name,
        color: input.color,
        note: input.note,
        liability: input.liability,
        contact_id: input.contact_id,
        secured_by: input.secured_by,
      })
      if (input.purpose) store.purpose = input.purpose
      if (input.binding) store.binding = input.binding
      if (input.custody) store.custody = input.custody
      if (input.property) store.property = input.property
      if (input.provider) {
        store.exchange = store.exchange
          ? { ...store.exchange, provider: input.provider }
          : { provider: input.provider, import_interval_sec: 900, terminal: mockTerminal() }
      }
      return resolve(store)
    }
    case 'archive_store': {
      // Like the backend: the whole subtree follows, and a child waits for its parent.
      const id = String(args?.id ?? '')
      const archived = Boolean(args?.archived)
      if (!archived && mockAncestors(id).some((parent) => parent.archived))
        throw { code: 'STORE_RESTORE_PARENT_FIRST', message: 'restore the parent first' }
      for (const target of [id, ...mockSubtreeIds(id)]) {
        const store = mockStore(target)
        if (store) store.archived = archived
      }
      return resolve({})
    }
    case 'delete_store': {
      const idx = mockStores.findIndex((candidate) => candidate.id === String(args?.id))
      if (idx >= 0) mockStores.splice(idx, 1)
      return resolve({})
    }
    // Tauri v2 converts primitive command args to camelCase, so the mock must
    // read the same names the real backend receives (data *fields* stay snake_case).
    case 'list_transactions':
      return resolve(mockListTxs((args?.filter as TxFilter | null) ?? emptyTxFilter(), mockExploreCtx()))
    case 'get_tx_graph':
      return resolve(
        mockTxGraph(
          (args?.filter as TxFilter | null) ?? emptyTxFilter(),
          (args?.collapsed as string[] | null) ?? [],
          mockExploreCtx(),
        ),
      )
    case 'get_store_overview':
      return resolve(mockOverview(String(args?.storeId ?? ''), args?.includeChildren !== false))
    case 'get_store_history':
      return resolve(mockHistory)
    case 'get_store_pnl':
      return resolve({ total_base: '1234.56', fees_base: '4.5', legs: [] })
    case 'get_summary':
      return resolve(mockSummary)
    case 'get_history':
      return resolve(mockHistory)
    case 'get_realized_pnl':
      return resolve({ total_base: '1234.56', fees_base: '4.5', legs: [] })
    case 'get_settings':
      return resolve(mockSettings)
    case 'update_settings': {
      const input = body as UpdateSettingsInput
      const settings = mockSettings
      // Like the backend: every field that is present is written, the rest is kept.
      for (const [key, value] of Object.entries(input)) {
        if (value !== null && value !== undefined) Object.assign(settings, { [key]: value })
      }
      return resolve(settings)
    }
    // --- app settings (outside the vault) ---
    case 'get_app_settings':
      return resolve(mockAppSettings)
    case 'update_app_settings': {
      const input = body as UpdateAppSettingsInput
      if (input.theme) mockAppSettings.theme = input.theme
      if (input.locale) mockAppSettings.locale = input.locale
      if (input.base_currency) mockAppSettings.base_currency = input.base_currency
      if (input.favorite_currencies) mockAppSettings.favorite_currencies = input.favorite_currencies
      if (input.timezone !== null && input.timezone !== undefined) mockAppSettings.timezone = input.timezone || null
      if (input.inflation) mockAppSettings.inflation = input.inflation
      if (input.enrichment_opt_in !== null && input.enrichment_opt_in !== undefined)
        mockAppSettings.enrichment_opt_in = input.enrichment_opt_in
      if (input.completed_tours) mockAppSettings.completed_tours = [...new Set(input.completed_tours)]
      return resolve(mockAppSettings)
    }
    case 'set_quick_add_shortcut': {
      const shortcut = String(args?.shortcut ?? '').trim()
      // The browser has no global keys; pretend one combination is taken to show the error.
      if (shortcut === 'Super+Space') throw { code: 'SHORTCUT_FAILED', message: 'already registered' }
      mockAppSettings.quick_add_shortcut = shortcut || null
      return resolve(mockAppSettings)
    }
    case 'get_supported_currencies':
      return resolve({ catalog: mockCurrencies, notice: null })
    case 'list_categories':
      return resolve(mockCategories)
    case 'create_category': {
      const input = body as CreateCategoryInput
      const key =
        (input.key || input.name)
          .trim()
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, '_') || `cat_${Date.now()}`
      const category: Category = { key, name: input.name.trim(), group: input.group, archived: false }
      mockCategories.push(category)
      return resolve(category)
    }
    case 'update_category': {
      const input = body as UpdateCategoryInput
      const category = mockCategories.find((entry) => entry.key === input.key)
      if (!category) throw { code: 'MISSING', message: 'category' }
      Object.assign(category, { name: input.name?.trim() || null, group: input.group })
      return resolve(category)
    }
    case 'archive_category': {
      const category = mockCategories.find((entry) => entry.key === String(args?.key))
      if (category) category.archived = Boolean(args?.archived)
      return resolve({})
    }
    case 'delete_category': {
      const key = String(args?.key)
      if (mockTransactions.some((tx) => txMeta(tx).categories.includes(key)))
        throw { code: 'INVALID_INPUT', message: 'the category is in use; archive it instead' }
      mockCategories.splice(
        mockCategories.findIndex((entry) => entry.key === key),
        1,
      )
      return resolve({})
    }
    case 'list_categorization_rules':
      return resolve([...mockRules].sort((left, right) => left.order - right.order))
    case 'create_categorization_rule': {
      const input = body as CreateRuleInput
      const rule = {
        id: crypto.randomUUID(),
        pattern: input.pattern,
        category: input.category,
        active: true,
        order: mockRules.length,
      }
      mockRules.push(rule)
      return resolve(rule)
    }
    case 'update_categorization_rule': {
      const input = body as UpdateRuleInput
      const rule = mockRules.find((entry) => entry.id === input.id)
      if (rule) Object.assign(rule, { pattern: input.pattern, category: input.category, active: input.active })
      return resolve(rule)
    }
    case 'delete_categorization_rule':
      mockRules.splice(
        mockRules.findIndex((entry) => entry.id === String(args?.id)),
        1,
      )
      return resolve({})
    case 'reorder_categorization_rules': {
      const ids = (args?.ids as string[]) ?? []
      for (const rule of mockRules) rule.order = ids.indexOf(rule.id) >= 0 ? ids.indexOf(rule.id) : ids.length
      return resolve({})
    }
    // --- contacts ---
    case 'list_contacts':
      return resolve(mockContacts)
    case 'create_contact': {
      const input = args?.input as ContactDraft
      const contact: Contact = {
        id: crypto.randomUUID(),
        name: input.name.trim(),
        kind: input.kind,
        note: input.note,
        accounts: input.accounts.map((account) => ({
          ...account,
          id: account.id || crypto.randomUUID(),
          active: true,
        })),
        archived: false,
        created_at: new Date().toISOString(),
      }
      mockContacts.push(contact)
      return resolve(contact)
    }
    case 'update_contact': {
      const input = body as UpdateContactInput
      const contact = mockContacts.find((entry) => entry.id === input.id)
      if (!contact) throw { code: 'MISSING', message: 'contact' }
      Object.assign(contact, {
        name: input.contact.name.trim(),
        kind: input.contact.kind,
        note: input.contact.note,
        accounts: input.contact.accounts.map((account) => ({
          ...account,
          id: account.id || crypto.randomUUID(),
          active: account.id ? account.active : true,
        })),
      })
      return resolve(contact)
    }
    case 'archive_contact': {
      const contact = mockContacts.find((entry) => entry.id === String(args?.id))
      if (contact) contact.archived = Boolean(args?.archived)
      return resolve({})
    }
    case 'delete_contact': {
      const id = String(args?.id)
      if (mockTransactions.some((tx) => txContactIds(tx).includes(id)))
        throw { code: 'INVALID_INPUT', message: 'the contact is used by transactions; archive it instead' }
      mockContacts.splice(
        mockContacts.findIndex((entry) => entry.id === id),
        1,
      )
      return resolve({})
    }
    case 'get_contact_turnover': {
      const input = body as ContactTurnoverInput
      return resolve(mockTurnover(mockTransactions, input.contact_id, input.account_id, txLegs))
    }
    case 'list_debt_terms': {
      const contactId = args?.contactId ? String(args.contactId) : null
      return resolve(mockDebtTerms.filter((entry) => !contactId || entry.contact_id === contactId))
    }
    case 'set_debt_terms': {
      const draft = body as DebtTermsDraft
      const asset = draft.asset.toUpperCase()
      const kept = mockDebtTerms.filter((entry) => !(entry.contact_id === draft.contact_id && entry.asset === asset))
      mockDebtTerms.splice(0, mockDebtTerms.length, ...kept)
      if (!draft.due_at && !draft.note.trim()) return resolve(null)
      const terms: DebtTerms = {
        id: crypto.randomUUID(),
        contact_id: draft.contact_id,
        asset,
        due_at: draft.due_at || null,
        note: draft.note.trim(),
        created_at: new Date().toISOString(),
      }
      mockDebtTerms.push(terms)
      return resolve(terms)
    }
    case 'get_contact_balances':
      return resolve(mockContactBalances(mockTransactions))
    case 'get_net_worth':
      return resolve(mockNetWorth())
    case 'set_manual_price': {
      const input = body as SetManualPriceInput
      return resolve({ ...input, updated_at: new Date().toISOString() })
    }
    case 'clear_manual_price':
      return resolve({})
    // --- transactions ---
    case 'transaction_kinds_for_store_kind':
      return resolve(mockKindsFor(args?.kind as StoreKind))
    case 'add_transaction':
      return resolve(mockPush(mockTransactionFrom(args?.input as TransactionInput, priceOf)))
    case 'preview_transaction':
      return resolve(mockPreview(body as PreviewTransactionInput))
    case 'update_transaction': {
      const input = body as UpdateTransactionInput
      const index = mockTransactions.findIndex((tx) => txId(tx) === input.id)
      if (index < 0) throw { code: 'MISSING', message: 'transaction' }
      const kept = { id: input.id, origin: txMeta(mockTransactions[index]).origin }
      mockTransactions[index] = mockTransactionFrom(input.transaction, priceOf, kept)
      return resolve(mockTransactions[index])
    }
    case 'delete_transaction': {
      const idx = mockTransactions.findIndex((tx) => txId(tx) === String(args?.id))
      if (idx >= 0) mockTransactions.splice(idx, 1)
      return resolve({})
    }
    case 'search_assets':
      return resolve(mockSearch(body?.query as string))
    case 'get_cached_price':
      return resolve(mockCachedPrice(body as AssetPriceInput))
    case 'convert_amount': {
      const input = body as ConvertAmountInput
      const from = priceOf(input.from)
      const to = priceOf(input.to)
      return resolve({ amount: from && to ? String((Number(input.amount) * from) / to) : null })
    }
    case 'calc_preview':
      return resolve(mockCalc(body as CalcInput))
    case 'ensure_fx':
      return resolve({ points: [], missing: [] } satisfies EnsureFxOutput)
    case 'refresh_all':
      return resolve({ prices: 3, fx: 1, restamped: 0, updated_at: new Date().toISOString() })
    case 'open_explorer':
      return resolve({})
    // --- exchange (read-only prefill) ---
    case 'inspect_tx':
      // Offline in the browser mock: no suggestions, form left untouched.
      return resolve({ fetched: false, info: null, hints: [], notice: 'unavailable' })
    case 'exchange_key_status':
      return resolve(mockKeyStatus)
    case 'set_exchange_key': {
      const input = body as SetExchangeKeyInput
      const apiKey = input.api_key
      mockKeyStatus = {
        ...mockKeyStatus,
        configured: true,
        hint: apiKey.length <= 8 ? '*'.repeat(apiKey.length) : `${apiKey.slice(0, 4)}…${apiKey.slice(-4)}`,
      }
      return resolve(mockKeyStatus)
    }
    case 'set_exchange_targets': {
      const input = body as SetExchangeTargetsInput
      mockKeyStatus = { ...mockKeyStatus, spot_store_id: input.spot_store_id, futures_store_id: input.futures_store_id }
      return resolve(mockKeyStatus)
    }
    case 'clear_exchange_key':
      mockKeyStatus = { configured: false, hint: null, spot_store_id: null, futures_store_id: null }
      return resolve(mockKeyStatus)
    case 'bybit_executions':
      return resolve(
        mockKeyStatus.configured ? { executions: mockExecutions, notice: null } : { executions: [], notice: 'no_key' },
      )
    case 'set_loyalty_info': {
      const input = body as SetLoyaltyInfoInput
      const store = mockStores.find((entry) => entry.id === input.store_id)
      if (!store) return Promise.reject({ code: 'MISSING', message: 'store' })
      store.loyalty = { ...input.info, points_symbol: input.info.points_symbol.toUpperCase() }
      mockPrices[store.loyalty.points_symbol] = Number(input.info.point_value) * (priceOf(input.info.currency) || 1)
      return resolve(store)
    }
    case 'adjust_points': {
      const input = body as AdjustPointsInput
      const store = mockStores.find((entry) => entry.id === input.store_id)
      const symbol = store?.loyalty?.points_symbol
      if (!store || !symbol) return Promise.reject({ code: 'INVALID_INPUT', message: 'set up the programme first' })
      const positive = Number(input.delta) > 0
      const tx = mockTransactionFrom(
        {
          type: positive ? 'topup' : 'payment',
          common: {
            timestamp: input.timestamp ?? new Date().toISOString(),
            comment: input.comment,
            categories: positive ? ['cashback'] : [],
            flow_id: null,
            contact_id: null,
            price_usd: null,
            fx_override: null,
            price: null,
            price_currency: null,
          },
          store_id: store.id,
          asset: { symbol, name: store.name, coingecko_id: null, chain: null, contract: null },
          amount: String(Math.abs(Number(input.delta))),
          fee: null,
          ...(positive ? { from: null } : { to: null }),
          details: {
            cash_breakdown: null,
            unit_price: null,
            network: null,
            tx_hash: null,
            source_store_id: input.source_store_id,
          },
        } as TransactionInput,
        priceOf,
      )
      return resolve(mockPush(tx))
    }
    case 'get_cashback_report': {
      const programmes: LoyaltyBalance[] = mockStores
        .filter((store) => store.kind === 'loyalty' && store.loyalty)
        .map((store) => {
          const points = mockOwnPositions(store.id).reduce((sum, position) => sum + Number(position.quantity), 0)
          return {
            store_id: store.id,
            name: store.name,
            points_symbol: store.loyalty?.points_symbol ?? '',
            points: String(points),
            value_base: String(points * priceOf(store.loyalty?.points_symbol ?? '')),
            expires_at: store.loyalty?.expires_at ?? null,
            expiring_points: store.loyalty?.expiring_points ?? null,
          }
        })
      const card = mockStores.find((store) => store.kind === 'bank')
      return resolve({
        base_currency: 'usd',
        cashback_base: '12.4',
        points_base: programmes.reduce((sum, row) => sum + Number(row.value_base), 0).toFixed(2),
        by_source: card ? [{ store_id: card.id, name: card.name, cashback_base: '12.4', points_base: '0' }] : [],
        programmes,
      } satisfies CashbackReport)
    }
    case 'set_deposit_terms': {
      const input = body as SetDepositTermsInput
      const store = mockStores.find((entry) => entry.id === input.store_id)
      if (!store) return Promise.reject({ code: 'MISSING', message: 'store' })
      store.deposit = input.terms ?? undefined
      return resolve(store)
    }
    case 'get_deposit_report': {
      const store = mockStores.find((entry) => entry.id === args?.storeId)
      const terms = store?.deposit
      const rate = terms?.rates.at(-1)?.rate ?? null
      const balance = mockOwnPositions(String(args?.storeId)).reduce(
        (sum, position) => sum + Number(position.quantity),
        0,
      )
      const yearly = rate ? (balance * Number(rate)) / 100 : 0
      const today = new Date()
      return resolve({
        store_id: String(args?.storeId),
        currency: 'rub',
        balance: String(balance),
        rate,
        rate_since: terms?.rates.at(-1)?.from ?? null,
        accrued_month: ((yearly / 365) * today.getDate()).toFixed(2),
        accrued_year: ((yearly / 365) * (today.getMonth() * 30 + today.getDate())).toFixed(2),
        posted_year: '0',
        projected_year: yearly.toFixed(2),
        matures_at: null,
        at_maturity: null,
        pds: terms?.pds
          ? {
              year: today.getFullYear(),
              contributions: String(balance),
              cofinancing: String(Math.min(36000, balance)),
              to_full_cofinancing: String(Math.max(0, 36000 - balance)),
              ratio: 1,
              cofinancing_due: `${today.getFullYear() + 1}-08-31`,
              ends_at: `${Number(terms.pds.first_contribution_at.slice(0, 4)) + 10}${terms.pds.first_contribution_at.slice(4)}`,
              deduction: String(Math.min(balance, 400000) * 0.13),
            }
          : null,
      } satisfies DepositReport)
    }
    case 'get_interest_tax':
      return resolve({
        year: new Date().getFullYear(),
        max_key_rate: '21',
        limit: '210000',
        income: '48250.5',
        taxable: '0',
        tax: '0',
        remaining_free: '161749.5',
      } satisfies InterestTaxReport)
    case 'refresh_key_rates':
      return resolve(3)
    case 'create_bybit_accounts': {
      const input = body as CreateBybitAccountsInput
      const root = mockStores.find((store) => store.id === input.store_id)
      if (!root) return Promise.reject({ code: 'MISSING', message: 'store' })
      const wanted: Array<[SubType, MarketKind, string]> = [
        ['funding', 'funding', input.funding_name],
        ['unified', 'unified', input.unified_name],
        ['earn', 'earn', input.earn_name],
      ]
      const ids = wanted.map(([subType, market, name]) => {
        const known = mockStores.find(
          (store) =>
            store.parent_id === root.id &&
            typeof store.binding === 'object' &&
            'market' in store.binding &&
            store.binding.market === market,
        )
        if (known) return known.id
        const created: Store = {
          ...newStore(name, 'exchange', root.color, root.id),
          sub_type: subType,
          binding: { market },
          exchange: null,
        }
        mockStores.push(created)
        return created.id
      })
      return resolve(ids)
    }
    case 'bybit_balances':
      return resolve(
        mockKeyStatus.configured
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
    // --- bybit import staging (D2.4) ---
    case 'import_bybit_history':
      return resolve(
        mockKeyStatus.configured
          ? { staged: mockPendingImports().length, notice: null }
          : { staged: 0, notice: 'no_key' },
      )
    case 'list_pending_imports':
      return resolve({ imports: mockPendingImports() })
    case 'confirm_pending_imports': {
      const input = body as ConfirmPendingImportsInput
      const ids = input.exec_ids.length ? input.exec_ids : mockPendingImports().map((entry) => entry.exec_id)
      const staged = mockPendingImports()
      mockPendingStore = staged.filter((entry) => !ids.includes(entry.exec_id))
      return resolve({ confirmed: staged.length - mockPendingStore.length })
    }
    case 'discard_pending_imports': {
      const input = body as DiscardPendingImportsInput
      const ids = input.exec_ids.length ? input.exec_ids : mockPendingImports().map((entry) => entry.exec_id)
      const staged = mockPendingImports()
      mockPendingStore = staged.filter((entry) => !ids.includes(entry.exec_id))
      return resolve({ discarded: staged.length - mockPendingStore.length })
    }
    case 'dev_dump_vault':
      return resolve({ json: JSON.stringify(mockVaultDoc(), null, 2) })
    case 'dev_export_vault':
      // No native save dialog in the browser mock; report a plausible path so
      // the success path stays observable during `pnpm dev`.
      return resolve('/tmp/mock/vault.json')
    case 'dev_apply_vault_json': {
      // Reject anything the editor would not accept, exactly like the backend.
      const text = String(args?.json ?? '')
      try {
        JSON.parse(text)
      } catch (err) {
        throw { code: 'DESERIALIZE', message: String(err) }
      }
      return resolve(JSON.stringify(JSON.parse(text), null, 2))
    }
    case 'dev_open_vault_json_in_editor':
      return resolve('/tmp/tradeapp-vault.json')
    case 'dev_reload_vault_json_from_editor':
      // Stands in for the scratch file: whatever the backend last reported.
      return resolve(mockScratchJson)
    case 'get_candles': {
      const input = body as GetCandlesInput
      return resolve(mockCandles(input))
    }
    case 'get_report':
      return resolve(mockReport(args?.kind as ReportKind, args?.filter as ReportFilter, analyticsContext()))
    case 'get_calendar':
      return resolve(mockCalendar(String(args?.month ?? new Date().toISOString().slice(0, 7)), analyticsContext()))
    case 'get_fx_table':
      return resolve(mockFxTable())
    case 'get_tx_loss':
      return resolve(mockLossOf(String(args?.id)))
    case 'get_tx_losses':
      return resolve(Object.fromEntries(mockTransactions.map((tx) => [txId(tx), mockLossOf(txId(tx))])))
    case 'list_flows':
      return resolve(mockFlows())
    case 'link_transactions': {
      const input = body as LinkTransactionsInput
      const flow =
        input.flow_id ??
        mockTransactions.map((tx) => (input.ids.includes(txId(tx)) ? txMeta(tx).flow_id : null)).find(Boolean) ??
        crypto.randomUUID()
      for (const tx of mockTransactions) if (input.ids.includes(txId(tx))) txMeta(tx).flow_id = flow
      return resolve(flow)
    }
    case 'unlink_transaction': {
      const tx = mockTransactions.find((entry) => txId(entry) === args?.id)
      if (tx) txMeta(tx).flow_id = null
      return resolve({})
    }
    case 'get_enrichments':
      return resolve([])
    case 're_enrich':
      return resolve(null)
    case 'apply_enrichment':
    case 'reject_enrichment':
      return Promise.reject({ code: 'MISSING', message: 'mock: no findings in the browser' })
    case 'export_report_csv':
      // No save dialog in the browser.
      return resolve(null)
    default: {
      const planned = mockPlanning(
        cmd,
        args,
        {
          transactions: mockTransactions,
          categories: mockCategories,
          stores: mockStores,
          priceOf,
          transactionFrom: (input, ts) =>
            mockTransactionFrom({ ...input, common: { ...input.common, timestamp: ts } } as TransactionInput, priceOf),
          budgetCurrency: mockSettings.budget_currency || mockAppSettings.base_currency,
        },
        mockSettings,
      )
      return resolve(planned ?? {})
    }
  }
}

/** Rough losses in the mock: the fee only, at the mock prices. */
function mockLossOf(id: string): TxLoss {
  const tx = mockTransactions.find((entry) => txId(entry) === id)
  const fee = tx
    ? txLegs(tx)
        .filter((leg) => leg.role === 'fee')
        .reduce((sum, leg) => sum + Math.abs(Number(leg.amount)) * priceOf(leg.asset.symbol), 0)
    : 0
  return {
    tx_id: id,
    currency: mockAppSettings.base_currency,
    fee: String(fee),
    slippage: '0',
    total: String(fee),
    internal: false,
    complete: true,
  }
}

function mockFlows(): FlowSummary[] {
  const groups = new Map<string, Transaction[]>()
  for (const tx of mockTransactions) {
    const flow = txMeta(tx).flow_id
    if (flow) groups.set(flow, [...(groups.get(flow) ?? []), tx])
  }
  return [...groups.entries()].map(([flowId, txs]) => {
    const links = [...txs].sort((left, right) => txMeta(left).timestamp.localeCompare(txMeta(right).timestamp))
    const loss = links.reduce((sum, tx) => sum + Number(mockLossOf(txId(tx)).total), 0)
    return {
      flow_id: flowId,
      currency: mockAppSettings.base_currency,
      links: links.map((tx) => ({ tx, loss: mockLossOf(txId(tx)) })),
      started_at: txMeta(links[0]).timestamp,
      ended_at: txMeta(links[links.length - 1]).timestamp,
      gave: [],
      got: [],
      gave_value: '0',
      got_value: '0',
      loss: String(loss),
      difference: String(loss),
    }
  })
}

function mockFxTable(): FxTable {
  const base = mockAppSettings.base_currency
  const perUsd: Record<string, number> = { usd: 1, rub: 95, eur: 0.92, thb: 36 }
  const baseRate = perUsd[base] ?? 1
  return {
    base,
    rows: Object.entries(perUsd)
      .filter(([code]) => code !== base)
      .map(([code, rate]) => ({
        currency: code,
        per_base: String(rate / baseRate),
        in_base: String(baseRate / rate),
        updated_at: new Date().toISOString(),
        tracked: code === 'rub',
      })),
  }
}

function analyticsContext() {
  return {
    transactions: mockTransactions,
    categories: mockCategories,
    stores: mockStores,
    priceOf,
    currency: mockSettings.budget_currency || mockAppSettings.base_currency,
  }
}

let mockKeyStatus: ExchangeKeyStatus = { configured: false, hint: null, spot_store_id: null, futures_store_id: null }

/** Scratch-file contents for the "pull from editor" mock round-trip. */
const mockScratchJson = ''

const mockExecutions: BybitExecution[] = [
  {
    symbol: 'BTCUSDT',
    side: 'Buy',
    order_type: 'Market',
    exec_price: '64250.5',
    exec_qty: '0.02',
    exec_value: '1285.01',
    exec_fee: '0.642505',
    exec_time: '1717000000000',
    is_maker: false,
    prefill: {
      side: 'buy',
      symbol: 'BTCUSDT',
      asset_sold: 'USDT',
      qty_sold: '1285.01',
      asset_bought: 'BTC',
      qty_bought: '0.02',
      price: '64250.5',
      fee: '0.642505',
      fee_asset: 'USDT',
      occurred_at: '2024-05-30T10:13:20Z',
      comment: 'bybit spot buy · 0.02 @ 64250.5',
    },
  },
]

/** Staged fills for the browser mock of the D2.4 import flow. */
let mockPendingStore: PendingImport[] = []
const mockKeyStatusStoreId = 'mock-store-bybit'

function mockPendingImports(storeId = mockKeyStatusStoreId): PendingImport[] {
  if (mockPendingStore.length) return mockPendingStore
  const now = Date.now()
  mockPendingStore = ['a1', 'a2'].map((suffix, index) => ({
    exec_id: `mock-exec-${suffix}`,
    store_id: storeId,
    category: 'spot',
    symbol: 'BTCUSDT',
    side: index === 0 ? 'buy' : 'sell',
    asset_bought: 'BTC',
    qty_bought: index === 0 ? '0.02' : '0',
    asset_sold: 'USDT',
    qty_sold: index === 0 ? '1285.01' : '1300',
    price: '64250.5',
    fee: '0.642505',
    fee_asset: 'USDT',
    occurred_at: new Date(now - index * 86_400_000).toISOString(),
    comment: `bybit spot ${index === 0 ? 'buy' : 'sell'} · mock`,
    is_maker: index === 0,
    closed_size: null,
    closed_pnl: null,
    kind: 'fill' as const,
    from_account: null,
    to_account: null,
  }))
  // A move between Bybit accounts and an Earn payout, like a real sweep stages.
  const quiet = { side: '', price: '0', fee: '0', fee_asset: '', is_maker: null, closed_size: null, closed_pnl: null }
  mockPendingStore.push(
    {
      ...quiet,
      exec_id: 'transfer:mock-1',
      store_id: storeId,
      category: 'transfer',
      symbol: 'USDT',
      asset_bought: 'USDT',
      qty_bought: '250',
      asset_sold: 'USDT',
      qty_sold: '250',
      occurred_at: new Date(now - 2 * 86_400_000).toISOString(),
      comment: 'Bybit: FUND → UNIFIED',
      kind: 'transfer',
      from_account: 'FUND',
      to_account: 'UNIFIED',
    },
    {
      ...quiet,
      exec_id: 'earn:mock-1',
      store_id: storeId,
      category: 'earn',
      symbol: 'USDT',
      asset_bought: 'USDT',
      qty_bought: '0.42',
      asset_sold: '',
      qty_sold: '0',
      occurred_at: new Date(now - 3 * 86_400_000).toISOString(),
      comment: 'Bybit Earn',
      kind: 'earn_income',
      from_account: null,
      to_account: 'EARN',
    },
  )
  return mockPendingStore
}

let mockStatus: VaultStatus = {
  path: '/tmp/mock/vault.wallet',
  exists: false,
  unlocked: false,
  info: null,
}

const mockInfo: VaultInfo = {
  path: '/tmp/mock/vault.wallet',
  doc_version: 1,
  id: crypto.randomUUID(),
  created_at: new Date().toISOString(),
  format: 'json',
  base_currency: 'usd',
}

const mockSettings: Settings = {
  display_decimals: 8,
  dashboard_widgets: [],
  fx_refresh_interval_sec: 300,
  minimize_to_tray: true,
  savings_rate: '0.10',
  daily_budget_mode: 'month',
  budget_currency: '',
}

/** Mirrors what `app_settings.json` would hold. */
const mockAppSettings: AppSettings = {
  theme: 'dark',
  locale: 'auto',
  base_currency: 'usd',
  favorite_currencies: [],
  timezone: null,
  inflation: { rub: '8', usd: '3', eur: '2.5', thb: '1' },
  enrichment_opt_in: null,
  quick_add_shortcut: null,
  completed_tours: [],
}

const mockCurrencies: SupportedCurrencies = {
  fiat: ['usd', 'eur', 'rub', 'cny', 'gbp', 'jpy', 'chf', 'cad', 'aud'],
  // Thousands of synthetic tickers, like the real CoinGecko list, so the picker's virtual scroll gets exercised.
  crypto: [
    'btc',
    'eth',
    'sol',
    'usdt',
    'bnb',
    'xrp',
    'doge',
    ...Array.from({ length: 5000 }, (_unused, index) => `tk${index.toString(36)}`),
  ],
}

/** Demo history: a salary, a payment, a trade, a debt and a conversion via an exchanger. */
function demoTransactions(): Transaction[] {
  const now = Date.now()
  const at = (hours: number) => new Date(now - hours * 3.6e6).toISOString()
  const common = (hours: number, comment: string, categories: string[] = []): TxCommonInput => ({
    timestamp: at(hours),
    comment,
    categories,
    flow_id: null,
    contact_id: null,
    price_usd: null,
    fx_override: null,
    price: null,
    price_currency: null,
  })
  const [, binance, ledger, futures, bank] = mockStores
  const btc = { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin', chain: 'bitcoin' as Chain, contract: null }
  const usdt = { symbol: 'USDT', name: 'Tether', coingecko_id: 'tether', chain: null, contract: null }
  const rub = { symbol: 'RUB', name: 'Рубль', coingecko_id: null, chain: 'fiat' as Chain, contract: null }
  const details: KindDetails = { cash_breakdown: null, unit_price: null, network: null, tx_hash: null }
  const inputs: TransactionInput[] = [
    {
      type: 'topup',
      common: common(240, 'Зарплата', ['salary']),
      store_id: bank.id,
      asset: rub,
      amount: '150000',
      fee: null,
      from: null,
      details,
    },
    {
      type: 'payment',
      common: common(200, 'Пятёрочка', ['groceries']),
      store_id: bank.id,
      asset: rub,
      amount: '3400',
      fee: null,
      to: { raw: 'Пятёрочка' },
      details,
    },
    {
      type: 'topup',
      common: common(190, 'Депозит', ['transfer']),
      store_id: binance.id,
      asset: usdt,
      amount: '5000',
      fee: null,
      from: null,
      details,
    },
    {
      type: 'trade_spot',
      common: common(150, 'Покупка BTC'),
      store_id: binance.id,
      sold: { asset: usdt, qty: '4500' },
      bought: { asset: btc, qty: '0.05' },
      fee: { asset: usdt, amount: '4.5' },
      liquidity: 'taker',
      order_id: null,
    },
    {
      type: 'transfer',
      common: common(120, 'На холодный'),
      from: { store: binance.id },
      to: { store: ledger.id },
      asset: btc,
      amount: '0.04',
      fee: { asset: btc, amount: '0.0002' },
      network: 'bitcoin',
      tx_hash: null,
      debt: null,
    },
    {
      type: 'transfer',
      common: common(100, 'Маржа на фьючи'),
      from: { store: binance.id },
      to: { store: futures.id },
      asset: usdt,
      amount: '400',
      fee: null,
      network: null,
      tx_hash: null,
      debt: null,
    },
    {
      type: 'transfer',
      common: common(48, 'Маме до зарплаты'),
      from: { store: bank.id },
      to: { contact: { contact_id: 'mock-contact-mom', account_id: 'mock-account-card' } },
      asset: rub,
      amount: '5000',
      fee: null,
      network: null,
      tx_hash: null,
      debt: 'lend',
    },
    {
      type: 'convert',
      common: { ...common(24, 'Обмен через skycapital'), contact_id: 'mock-contact-sky' },
      from: { account: { store: bank.id }, asset: rub, amount: '45000' },
      to: { account: { store: binance.id }, asset: usdt, amount: '480' },
      fee: null,
      rate_override: '93.75',
    },
  ]
  return inputs.map((input) => mockTransactionFrom(input, priceOf))
}

const newStore = (name: string, kind: StoreKind, color: string, parent: string | null = null, note = ''): Store => ({
  id: crypto.randomUUID(),
  name,
  kind,
  color,
  archived: false,
  note,
  parent_id: parent,
  contact_id: null,
  liability: null,
  sub_type: null,
  binding: 'any',
  purpose: kind === 'cash' || kind === 'bank' ? 'everyday' : 'investment',
  custody: kind === 'crypto' ? 'cold' : null,
  property: null,
  secured_by: null,
  exchange: kind === 'exchange' ? { provider: 'bybit', import_interval_sec: 900, terminal: mockTerminal() } : null,
})

const mockEnvelopes: Envelope[] = []
const mockDebtTerms: DebtTerms[] = []

function mockEnvelopeView(envelope: Envelope): EnvelopeView {
  const held = mockOwnPositions(envelope.store_id)
    .filter((position) => position.asset.symbol.toUpperCase() === envelope.asset.symbol.toUpperCase())
    .reduce((sum, position) => sum + Number(position.quantity), 0)
  const earmarked = mockEnvelopes
    .filter((entry) => entry.store_id === envelope.store_id && entry.asset.symbol === envelope.asset.symbol)
    .reduce((sum, entry) => sum + Number(entry.allocated), 0)
  const free = held - earmarked
  const target = Number(envelope.target ?? 0)
  return {
    envelope,
    free: String(free),
    pct: target ? (Number(envelope.allocated) / target) * 100 : null,
    short: free < 0,
  }
}

/** Mirrors `AutomationPreset::tx_kind` / `category` in Rust. */
function mockPresets(names: AutomationPreset[]): PresetInfo[] {
  const kinds: Record<AutomationPreset, TxKind> = {
    expense: 'payment',
    top_up: 'transfer',
    salary: 'topup',
    card_payment: 'payment',
    to_savings: 'transfer',
    deposit_interest: 'topup',
    dca_buy: 'convert',
    staking_reward: 'topup',
    market_transfer: 'transfer',
    rent_income: 'topup',
    property_tax: 'payment',
    insurance: 'payment',
    utilities: 'payment',
    loan_payment: 'transfer',
  }
  const categories: Partial<Record<AutomationPreset, string>> = {
    salary: 'salary',
    deposit_interest: 'deposit_interest',
    staking_reward: 'income_other',
    rent_income: 'income_other',
    to_savings: 'savings',
    dca_buy: 'trading',
    market_transfer: 'transfer',
    top_up: 'transfer',
    loan_payment: 'loan_payment',
    property_tax: 'taxes',
    insurance: 'insurance',
    utilities: 'utilities',
  }
  return names.map((preset) => ({ preset, tx_kind: kinds[preset], category: categories[preset] ?? null }))
}

/** Mirrors `WalletCatalog::build` in Rust. */
function mockWalletCatalog(): WalletCatalog {
  const sub = (sub_type: SubType, shape: BindingShapeName, purpose: Purpose | null = null): SubTypeInfo => ({
    sub_type,
    shape,
    default_purpose: purpose,
    credit: sub_type === 'credit_card',
  })
  return {
    kinds: [
      {
        kind: 'cash',
        presets: mockPresets(['expense', 'top_up']),
        max_roots: 1,
        user_creatable: false,
        default_purpose: 'everyday',
        sub_types: [sub('pocket', 'asset', 'everyday')],
      },
      {
        kind: 'bank',
        presets: mockPresets(['salary', 'card_payment', 'to_savings', 'deposit_interest']),
        max_roots: null,
        user_creatable: true,
        default_purpose: 'everyday',
        sub_types: [
          sub('card', 'asset', 'everyday'),
          sub('account', 'asset', 'everyday'),
          sub('deposit', 'asset', 'savings'),
          sub('savings', 'asset', 'savings'),
          sub('pds', 'asset', 'savings'),
          sub('credit_card', 'asset', 'everyday'),
        ],
      },
      {
        kind: 'crypto',
        presets: mockPresets(['dca_buy', 'staking_reward']),
        max_roots: null,
        user_creatable: true,
        default_purpose: 'investment',
        sub_types: [sub('token', 'asset')],
      },
      {
        kind: 'exchange',
        presets: mockPresets(['dca_buy', 'market_transfer']),
        max_roots: null,
        user_creatable: true,
        default_purpose: 'investment',
        sub_types: [
          sub('funding', 'market'),
          sub('unified', 'market'),
          sub('earn', 'market'),
          sub('spot', 'market'),
          sub('futures', 'market'),
          sub('pair', 'pair'),
        ],
      },
      {
        kind: 'property',
        presets: mockPresets(['rent_income', 'property_tax', 'insurance', 'utilities']),
        max_roots: null,
        user_creatable: true,
        default_purpose: 'investment',
        sub_types: [],
      },
      {
        kind: 'loyalty',
        presets: [],
        max_roots: null,
        user_creatable: true,
        default_purpose: 'savings',
        sub_types: [],
      },
      {
        kind: 'liability',
        presets: mockPresets(['loan_payment']),
        max_roots: null,
        user_creatable: true,
        default_purpose: 'investment',
        sub_types: [],
      },
    ],
    providers: [
      { provider: 'bybit', supported: true },
      { provider: 'binance', supported: false },
      { provider: 'okx', supported: false },
    ],
  }
}

function mockTerminal(): TerminalState {
  return {
    watchlist: ['BTCUSDT', 'ETHUSDT'],
    active_symbol: null,
    timeframe: '1h',
    indicators: [],
    drawings: {},
    layout: { show_watchlist: true, watchlist_width: 240 },
  }
}

function mockMarketBinding(subType: SubType): Binding {
  if (['spot', 'futures', 'earn', 'funding', 'unified'].includes(subType)) return { market: subType as MarketKind }
  return 'any'
}

function mockDefaultPurpose(input: CreateStoreInput): Purpose {
  if (input.sub_type === 'deposit' || input.sub_type === 'savings') return 'savings'
  if (input.sub_type && ['card', 'credit_card', 'account', 'pocket'].includes(input.sub_type)) return 'everyday'
  const parent = input.parent_id ? mockStore(input.parent_id) : undefined
  if (parent) return parent.purpose
  return input.kind === 'cash' || input.kind === 'bank' ? 'everyday' : 'investment'
}

const mockStores: Store[] = [
  newStore('Наличные', 'cash', '#16a34a'),
  newStore('Bybit', 'exchange', '#f0b90b', null, 'Фьючи'),
  newStore('Ledger', 'crypto', '#3b82f6'),
]
// A sub-account, so the wallet tree has something to show in the browser.
mockStores.push({
  ...newStore('Futures', 'exchange', '#f0b90b', mockStores[1].id),
  sub_type: 'futures',
  binding: { market: 'futures' },
  exchange: null,
})
mockStores.push(newStore('Тинькофф', 'bank', '#facc15'))
// A home with bills, so the utilities screens have something to show.
mockStores.push({
  ...newStore('Квартира', 'property', '#a855f7'),
  property: {
    category: 'flat',
    address: '',
    area: null,
    purchased_at: null,
    purchase_price: null,
    currency: 'rub',
    utilities_pay_day: 10,
  },
})
seedUtilityDemo(mockStores[mockStores.length - 1].id)

const mockTransactions: Transaction[] = []

/** Last known USD prices, mirroring what the Rust `price_cache` would hold. */
const mockPrices: Record<string, number> = {
  BTC: 90000,
  ETH: 3000,
  USD: 1,
  USDT: 1,
  RUB: 0.0105,
  EUR: 1.08,
  THB: 0.028,
}

const priceOf = (symbol: string) => mockPrices[symbol.toUpperCase()] ?? 0

const timestampOf = (tx: Transaction) => txMeta(tx).timestamp

function mockFilterTxs(storeIds: string[] | null): Transaction[] {
  const list = storeIds
    ? mockTransactions.filter((tx) => txLegs(tx).some((leg) => storeIds.includes(leg.store_id)))
    : mockTransactions
  return [...list].sort((left, right) => timestampOf(right).localeCompare(timestampOf(left)))
}

function mockPush(tx: Transaction): Transaction {
  mockTransactions.push(tx)
  return tx
}

/** Fill the demo history once every helper above is defined. */
function seedMock() {
  if (!mockTransactions.length) mockTransactions.push(...demoTransactions())
}

/** Mirrors `MAX_STORE_DEPTH` in Rust: a root is depth 0, so two levels. */
const MOCK_MAX_DEPTH = 1

const mockStore = (id: string) => mockStores.find((candidate) => candidate.id === id)
const mockChildren = (id: string) => mockStores.filter((candidate) => candidate.parent_id === id)

/** Parent chain from the root down to `id`'s parent. */
function mockAncestors(id: string): Store[] {
  const chain: Store[] = []
  let parent = mockStore(id)?.parent_id ?? null
  while (parent) {
    const store = mockStore(parent)
    if (!store || chain.includes(store)) break
    chain.unshift(store)
    parent = store.parent_id
  }
  return chain
}

function mockSubtreeIds(id: string): string[] {
  return mockChildren(id).flatMap((child) => [child.id, ...mockSubtreeIds(child.id)])
}

function mockHeight(id: string): number {
  return Math.max(0, ...mockChildren(id).map((child) => mockHeight(child.id) + 1))
}

/** Same rules and codes as `validate_placement` in Rust (R1–R6). */
function mockPlacementError(node: string | null, parent: string | null, name: string): AppError | null {
  if (parent) {
    const target = mockStore(parent)
    if (!target) return { code: 'MISSING', message: 'parent store' }
    if (node && !mockStore(node)?.parent_id) return { code: 'STORE_ROOT_MOVE', message: 'root move' }
    if (node && (parent === node || mockSubtreeIds(node).includes(parent)))
      return { code: 'STORE_CYCLE', message: 'cycle' }
    if (mockAncestors(parent).length + 1 + (node ? mockHeight(node) : 0) > MOCK_MAX_DEPTH)
      return { code: 'STORE_TOO_DEEP', message: 'too deep' }
    if (target.archived) return { code: 'STORE_PARENT_ARCHIVED', message: 'parent archived' }
  }
  const wanted = name.trim().toLowerCase()
  const taken = mockStores.some(
    (store) => store.id !== node && store.parent_id === parent && store.name.trim().toLowerCase() === wanted,
  )
  return taken ? { code: 'STORE_NAME_TAKEN', message: 'name taken' } : null
}

/** Positions held on the store itself, like `store_positions` in Rust. */
function mockOwnPositions(storeId: string): StorePosition[] {
  const byAsset = new Map<string, { asset: AssetRef; qty: number; cost: number }>()
  // Legs carry their own sign, so a trade or transfer needs no special case.
  for (const leg of mockTransactions.flatMap(txLegs)) {
    if (leg.store_id !== storeId || leg.asset.contract?.startsWith('perp:')) continue
    const key = leg.asset.symbol.toUpperCase()
    const row = byAsset.get(key) ?? { asset: leg.asset, qty: 0, cost: 0 }
    row.qty += Number(leg.amount)
    if (Number(leg.amount) > 0) row.cost = priceOf(key)
    byAsset.set(key, row)
  }
  return [...byAsset.values()]
    .filter((row) => row.qty !== 0)
    .map((row) => {
      const priceUsd = priceOf(row.asset.symbol)
      return {
        asset: row.asset,
        quantity: String(row.qty),
        avg_cost: row.cost ? String(row.cost) : null,
        price_usd: String(priceUsd),
        price_base: String(priceUsd),
        value_usd: String(row.qty * priceUsd),
        value_base: String(row.qty * priceUsd),
        unrealized_base: String(row.qty * (priceUsd - row.cost)),
      }
    })
    .sort((left, right) => Number(right.value_base) - Number(left.value_base))
}

/** Like `preview_transaction` in Rust: net legs per store and asset, nothing written. */
function mockPreview(input: PreviewTransactionInput): TxPreview {
  // Like the backend: a draft that would be refused (no category, no comment)
  // still previews its balances, with the refusal reported in `error`.
  let error: string | null = null
  let draft: Transaction
  try {
    draft = mockTransactionFrom(input.transaction, priceOf)
  } catch (err) {
    const failure = err as { code?: string; message?: string }
    error = `${failure.code ?? 'INVALID_INPUT'}: ${failure.message ?? ''}`
    const common = input.transaction.common
    const patched = {
      ...input.transaction,
      common: {
        ...common,
        comment: common.comment || 'preview',
        categories: common.categories.length ? common.categories : ['other'],
      },
    } as TransactionInput
    try {
      draft = mockTransactionFrom(patched, priceOf)
    } catch {
      return { changes: [], base_currency: mockSummary.base_currency, flow: 'ignored', warnings: [], error }
    }
  }
  const replaced = input.replaces ? mockTransactions.find((tx) => txId(tx) === input.replaces) : undefined
  const deltas = new Map<string, { storeId: string; asset: AssetRef; delta: number }>()
  const add = (legs: ReturnType<typeof txLegs>, sign: number) => {
    for (const leg of legs) {
      const key = `${leg.store_id}:${leg.asset.symbol.toUpperCase()}`
      const row = deltas.get(key) ?? { storeId: leg.store_id, asset: leg.asset, delta: 0 }
      row.delta += sign * Number(leg.amount)
      deltas.set(key, row)
    }
  }
  add(txLegs(draft), 1)
  if (replaced) add(txLegs(replaced), -1)
  const changes = [...deltas.values()]
    .filter((row) => row.delta !== 0)
    .map((row) => {
      const held = mockOwnPositions(row.storeId).find(
        (position) => position.asset.symbol.toUpperCase() === row.asset.symbol.toUpperCase(),
      )
      const before = Number(held?.quantity ?? 0)
      return {
        store_id: row.storeId,
        store_name: mockStore(row.storeId)?.name ?? '',
        asset: row.asset,
        before: String(before),
        after: String(before + row.delta),
        delta_base: String(row.delta * priceOf(row.asset.symbol)),
      }
    })
  const warnings: PreviewWarning[] = changes
    .filter((change) => Number(change.after) < 0 && mockStore(change.store_id)?.kind !== 'liability')
    .map((change) => ({ kind: 'negative_balance', store_id: change.store_id, asset: change.asset }))
  const flow: FlowClass = 'payment' in draft ? 'daily' : 'topup' in draft ? 'income' : 'ignored'
  return { changes, base_currency: mockSummary.base_currency, flow, warnings, error }
}

const sumValue = (positions: StorePosition[]) => positions.reduce((acc, row) => acc + Number(row.value_usd), 0)

function mockTreeNode(store: Store, depth: number, parentPath: string[]): StoreTreeNode {
  const path = [...parentPath, store.name]
  const children = mockChildren(store.id)
    .map((child) => mockTreeNode(child, depth + 1, path))
    .sort((left, right) => left.store.name.localeCompare(right.store.name))
  const own = sumValue(mockOwnPositions(store.id))
  const subtree = children.reduce((acc, child) => acc + Number(child.subtree_usd), own)
  return {
    store,
    depth,
    path,
    own_usd: String(own),
    own_base: String(own),
    subtree_usd: String(subtree),
    subtree_base: String(subtree),
    own_tx_count: mockFilterTxs([store.id]).length,
    can_add_child:
      depth < MOCK_MAX_DEPTH && !store.archived && !['property', 'cash', 'loyalty', 'liability'].includes(store.kind),
    children,
  }
}

function mockTree(): StoreTreeNode[] {
  return mockStores
    .filter((store) => !store.parent_id || !mockStore(store.parent_id))
    .map((store) => mockTreeNode(store, 0, []))
    .sort((left, right) => left.store.name.localeCompare(right.store.name))
}

function mockFindNode(nodes: StoreTreeNode[], id: string): StoreTreeNode | undefined {
  for (const node of nodes) {
    if (node.store.id === id) return node
    const found = mockFindNode(node.children, id)
    if (found) return found
  }
  return undefined
}

const flatNodes = (nodes: StoreTreeNode[]): StoreTreeNode[] =>
  nodes.flatMap((node) => [{ ...node, children: [] }, ...flatNodes(node.children)])

function mockMoveTargets(id: string): StoreMoveTargets {
  const store = mockStore(id)
  if (!store?.parent_id) return { parents: [], can_detach: false }
  return {
    parents: flatNodes(mockTree()).filter(
      (node) => node.store.id !== store.parent_id && !mockPlacementError(id, node.store.id, store.name),
    ),
    can_detach: !mockPlacementError(id, null, store.name),
  }
}

/** Every id a store scope covers. */
const mockScope = (storeId: string, includeChildren: boolean) =>
  includeChildren ? [storeId, ...mockSubtreeIds(storeId)] : [storeId]

const mockExploreCtx = () => ({
  transactions: mockTransactions,
  tree: mockTree(),
  contacts: mockContacts,
  scope: mockScope,
})

function mockOverview(storeId: string, includeChildren: boolean): StoreOverview {
  const store = mockStore(storeId) ?? mockStores[0]
  const node = mockFindNode(mockTree(), store.id) ?? mockTreeNode(store, 0, [])
  const txs = mockFilterTxs(mockScope(store.id, includeChildren))
  // Merged per asset like `merged_positions` in Rust (cost basis left out).
  const merged = new Map<string, StorePosition>()
  for (const row of [store.id, ...mockSubtreeIds(store.id)].flatMap(mockOwnPositions)) {
    const key = row.asset.symbol.toUpperCase()
    const prev = merged.get(key)
    const add = (left: string, right: string) => String(Number(left) + Number(right))
    merged.set(
      key,
      prev
        ? {
            ...prev,
            avg_cost: null,
            quantity: add(prev.quantity, row.quantity),
            value_usd: add(prev.value_usd, row.value_usd),
            value_base: add(prev.value_base, row.value_base),
            unrealized_base: add(prev.unrealized_base, row.unrealized_base),
          }
        : row,
    )
  }
  const subtreePositions = [...merged.values()].sort(
    (left, right) => Number(right.value_base) - Number(left.value_base),
  )
  const subtreeTotal = Number(node.subtree_usd)
  return {
    store,
    ancestors: mockAncestors(store.id).map((parent) => ({ id: parent.id, name: parent.name, color: parent.color })),
    base_currency: mockAppSettings.base_currency,
    fx_per_usd: '1',
    include_children: includeChildren,
    own_usd: node.own_usd,
    own_base: node.own_base,
    subtree_usd: node.subtree_usd,
    subtree_base: node.subtree_base,
    realized_pnl_base: '0',
    positions: mockOwnPositions(store.id),
    subtree_positions: subtreePositions,
    open_positions: [],
    children: node.children.map((child) => ({
      store: child.store,
      value_usd: child.own_usd,
      value_base: child.own_base,
      subtree_base: child.subtree_base,
      share: subtreeTotal ? Number(child.subtree_usd) / subtreeTotal : 0,
    })),
    can_add_child: node.can_add_child,
    tx_count: txs.length,
    first_tx_at: txs.length ? timestampOf(txs[txs.length - 1]) : '',
    last_tx_at: txs.length ? timestampOf(txs[0]) : '',
  }
}

function mockCachedPrice(input: AssetPriceInput): CachedPrice {
  const price = priceOf(input.symbol)
  if (!price) return { price_usd: '0', ts: '' }
  return { price_usd: String(price), ts: input.at || new Date().toISOString() }
}

function mockCalc(input: CalcInput): CalcResult {
  const num = (value: string | null | undefined) => Number(value ?? 0)
  const amount = num(input.amount)
  const priceIn = num(input.price_in)
  const priceOut = num(input.price_out)
  const feePct = num(input.fee_pct)
  if (!amount || !priceIn) {
    return {
      gross_in: '0',
      fee: '0',
      spent: '0',
      gross_out: '0',
      net_out: '0',
      exit_value: '0',
      pnl: '0',
      roi_pct: '0',
      risk: '0',
      risk_pct: '0',
    }
  }
  const zero = (): CalcResult => ({
    gross_in: '0',
    fee: '0',
    spent: '0',
    gross_out: '0',
    net_out: '0',
    exit_value: '0',
    pnl: '0',
    roi_pct: '0',
    risk: '0',
    risk_pct: '0',
  })
  const gross_in = amount * priceIn
  const fee = (gross_in * feePct) / 100
  const spent = gross_in + fee
  if (input.mode === 'converter') {
    if (!priceOut) return { ...zero(), gross_in: String(gross_in), fee: String(fee), spent: String(spent) }
    const gross_out = amount * priceOut
    return {
      ...zero(),
      gross_in: String(gross_in),
      fee: String(fee),
      spent: String(spent),
      gross_out: String(gross_out),
      net_out: String(gross_out - fee),
    }
  }
  const out: CalcResult = { ...zero(), gross_in: String(gross_in), fee: String(fee), spent: String(spent) }
  if (input.target_price) {
    const exit = amount * Number(input.target_price)
    const pnl = exit - spent
    out.exit_value = String(exit)
    out.pnl = String(pnl)
    out.roi_pct = String((pnl / spent) * 100)
  }
  if (input.stop_price) {
    out.risk = String(amount * Math.max(priceIn - Number(input.stop_price), 0))
    out.risk_pct = String((Number(out.risk) / spent) * 100)
  }
  return out
}

const mockSummary: PortfolioSummary = {
  base_currency: 'usd',
  base_fx_as_of: '',
  fx_per_usd: '1',
  total_usd: '42000',
  total_base: '42000',
  assets: [
    {
      asset: {
        symbol: 'BTC',
        name: 'Bitcoin',
        coingecko_id: 'bitcoin',
        chain: 'bitcoin',
        contract: null,
        quote: 'BTC',
      },
      quantity: '3',
      price_usd: '9000',
      value_usd: '27000',
      price_base: '9000',
      value_base: '27000',
    },
    {
      asset: {
        symbol: 'ETH',
        name: 'Ethereum',
        coingecko_id: 'ethereum',
        chain: 'ethereum',
        contract: null,
        quote: 'ETH',
      },
      quantity: '50',
      price_usd: '300',
      value_usd: '15000',
      price_base: '300',
      value_base: '15000',
    },
  ],
  by_chain: [
    { label: 'Bitcoin', value_usd: '27000', value_base: '27000', pct: 64.3 },
    { label: 'Ethereum', value_usd: '15000', value_base: '15000', pct: 35.7 },
  ],
  by_store: [
    { store_id: mockStores[1].id, name: 'Binance', value_usd: '30000', value_base: '30000', pct: 71 },
    { store_id: mockStores[2].id, name: 'Ledger', value_usd: '12000', value_base: '12000', pct: 29 },
  ],
  assets_base: '42050',
  liabilities_base: '0',
  net_worth_base: '42050',
  prices_as_of: new Date(Date.now() - 5 * 60_000).toISOString(),
}

function mockNetWorth(): NetWorth {
  const financial = Number(mockSummary.total_base)
  const receivables = mockContactBalances(mockTransactions).reduce(
    (acc, row) => acc + Number(row.receivable) * priceOf(row.asset.symbol),
    0,
  )
  return {
    base_currency: 'usd',
    financial: String(financial),
    non_financial: '0',
    receivables: String(receivables),
    liabilities: '0',
    payables: '0',
    net: String(financial + receivables),
  }
}

const mockHistory: HistoryPoint[] = [
  { ts: '2026-08-01T00:00:00Z', ts_ms: 1782864000000n, value_usd: '20000', value_base: '20000' },
  { ts: '2026-09-01T00:00:00Z', ts_ms: 1785542400000n, value_usd: '32000', value_base: '32000' },
  { ts: '2026-09-28T00:00:00Z', ts_ms: 1787961600000n, value_usd: '42000', value_base: '42000' },
]

function mockSearch(rawQuery: string): AssetSearchHit[] {
  const known = [
    { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin' },
    { symbol: 'ETH', name: 'Ethereum', coingecko_id: 'ethereum' },
    { symbol: 'SOL', name: 'Solana', coingecko_id: 'solana' },
    { symbol: 'USDT', name: 'Tether', coingecko_id: 'tether' },
  ]
  const needle = (rawQuery ?? '').toLowerCase()
  return known
    .filter((entry) => entry.symbol.toLowerCase().includes(needle) || entry.name.toLowerCase().includes(needle))
    .map((entry) => ({ ...entry, chain: 'ethereum', contract: null, rank: 1n }))
}

function mockCandles(input: GetCandlesInput): GetCandlesOutput {
  const limit = Math.min(Math.max(input.limit || 300, 1), 1000)
  const stepMs = (input.interval_min || 60) * 60_000
  const now = Math.floor(Date.now() / stepMs) * stepMs
  const basePrice = 60000 + ((input.symbol.length * 1500) % 30000)
  let price = basePrice
  const candles: CandlePoint[] = []
  for (let step = limit - 1; step >= 0; step--) {
    const drift = Math.sin(step / 17) * 0.004 + Math.cos(step / 6) * 0.002
    const open = price
    const close = open * (1 + drift)
    const high = Math.max(open, close) * (1 + 0.002 * ((step * 7) % 5))
    const low = Math.min(open, close) * (1 - 0.002 * ((step * 3) % 5))
    const volume = String(50 + ((step * 37) % 200))
    candles.push({
      ts: new Date(now - step * stepMs).toISOString(),
      open: open.toFixed(2),
      high: high.toFixed(2),
      low: low.toFixed(2),
      close: close.toFixed(2),
      volume,
    })
    price = close
  }
  return { candles }
}

/** Stand-in for the decrypted `VaultDoc`, mirroring the Rust dump command. */
function mockVaultDoc(): VaultDoc {
  return {
    version: 1,
    id: mockInfo.id,
    created_at: mockInfo.created_at,
    settings: mockSettings,
    stores: mockStores,
    holdings: [],
    transactions: mockTransactions,
    categories: mockCategories,
    price_cache: [],
    fx_cache: [],
    pending_imports: [],
    contacts: mockContacts,
    automations: [],
    automation_runs: [],
    budgets: [],
    goals: [],
    reminders: [],
    price_alerts: [],
    categorization_rules: mockRules,
    utility_items: [],
    meter_readings: [],
    enrichments: [],
    manual_prices: [],
    envelopes: mockEnvelopes,
    debt_terms: mockDebtTerms,
    key_rates: [],
  }
}

function resolve<T>(value: T): Promise<T> {
  return new Promise((res) => setTimeout(() => res(structuredClone(value)), 30))
}
