/** Commands: the wallet tree - create, move, change, archive, delete. */

import { mockTransactionFrom } from './ledger'
import { mockImportSettings, mockTerminal } from './catalog'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockDefaultPurpose, mockMarketBinding, mockPush, mockStores, mockTransactions, priceOf } from './demo'
import { mockAncestors, mockMoveTargets, mockPlacementError, mockStore, mockSubtreeIds, mockTree } from './tree'

/** Commands: the wallet tree: create, move, change, archive. */
export const storeTreeCommands: MockCommands = {
  get_store_tree: (_call) => {
    return resolve(mockTree())
  },
  store_move_targets: ({ args }) => {
    return resolve(mockMoveTargets(String(args?.id ?? '')))
  },
  move_store: ({ body }) => {
    const input = body as MoveStoreInput
    const store = mockStore(input.id)
    if (!store) throw { code: 'MISSING', message: 'store' }
    const failure = mockPlacementError(input.id, input.parent_id, store.name)
    if (failure) throw failure
    store.parent_id = input.parent_id
    return resolve(store)
  },
  create_store: ({ body }) => {
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
        ? {
            provider: input.provider,
            import_interval_sec: 900,
            terminal: mockTerminal(),
            import: mockImportSettings(),
          }
        : null,
    }
    mockStores.push(store)
    // Like `create_bank_with_card`: a new bank gets its first card, and the
    // opening balance goes on it.
    let openingStoreId = store.id
    if (input.card && input.kind === 'bank' && !input.parent_id) {
      const code = input.card.currency.toUpperCase()
      const card = {
        ...store,
        id: crypto.randomUUID(),
        name: input.card.name.trim() || `${input.name} ${code}`,
        parent_id: store.id,
        sub_type: 'card' as SubType,
        binding: {
          asset: {
            symbol: code,
            name: code,
            coingecko_id: null,
            chain: 'fiat' as Chain,
            contract: null,
            quote: code,
          },
        },
      }
      mockStores.push(card)
      openingStoreId = card.id
    }
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
            store_id: openingStoreId,
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
  },
  plan_store_change: ({ cmd, body }) => {
    // A thin stand-in for `store_change.rs`: no conflict checks, the shape only.
    const input = body as StoreChangeInput
    const store = mockStore(input.store_id)
    if (!store) throw { code: 'MISSING', message: 'store' }
    const item = (entry: Store, before: SubType | null) => ({
      store_id: entry.id,
      name: entry.name,
      kind_after: entry.kind,
      sub_type_before: before,
      sub_type_after: entry.sub_type,
    })
    const touching = mockTransactions.filter((tx) => JSON.stringify(tx).includes(store.id)).length
    const plan: StoreChangePlan = {
      store_id: store.id,
      kept: [item(store, store.sub_type)],
      archived: [],
      conflicts: [],
      moved_transactions: typeof input.change === 'object' && 'merge_into' in input.change ? touching : 0,
      error: null,
    }
    if (cmd === 'plan_store_change') return resolve(plan)
    const change = input.change
    if (change === 'detach') Object.assign(store, { parent_id: null, sub_type: null, binding: 'any' })
    else if ('change_kind' in change) {
      store.kind = change.change_kind.kind
      for (const child of mockStores.filter((entry) => entry.parent_id === store.id)) child.kind = store.kind
    } else if ('move_to' in change) {
      const parent = mockStore(change.move_to.parent_id)
      if (parent) Object.assign(store, { parent_id: parent.id, kind: parent.kind })
    } else if ('merge_into' in change) {
      const target = change.merge_into.target_id
      mockStores.splice(mockStores.indexOf(store), 1)
      const moved = JSON.parse(JSON.stringify(mockTransactions).replaceAll(store.id, target)) as Transaction[]
      mockTransactions.splice(0, mockTransactions.length, ...moved)
    }
    return resolve(plan)
  },
  apply_store_change: ({ cmd, body }) => {
    // A thin stand-in for `store_change.rs`: no conflict checks, the shape only.
    const input = body as StoreChangeInput
    const store = mockStore(input.store_id)
    if (!store) throw { code: 'MISSING', message: 'store' }
    const item = (entry: Store, before: SubType | null) => ({
      store_id: entry.id,
      name: entry.name,
      kind_after: entry.kind,
      sub_type_before: before,
      sub_type_after: entry.sub_type,
    })
    const touching = mockTransactions.filter((tx) => JSON.stringify(tx).includes(store.id)).length
    const plan: StoreChangePlan = {
      store_id: store.id,
      kept: [item(store, store.sub_type)],
      archived: [],
      conflicts: [],
      moved_transactions: typeof input.change === 'object' && 'merge_into' in input.change ? touching : 0,
      error: null,
    }
    if (cmd === 'plan_store_change') return resolve(plan)
    const change = input.change
    if (change === 'detach') Object.assign(store, { parent_id: null, sub_type: null, binding: 'any' })
    else if ('change_kind' in change) {
      store.kind = change.change_kind.kind
      for (const child of mockStores.filter((entry) => entry.parent_id === store.id)) child.kind = store.kind
    } else if ('move_to' in change) {
      const parent = mockStore(change.move_to.parent_id)
      if (parent) Object.assign(store, { parent_id: parent.id, kind: parent.kind })
    } else if ('merge_into' in change) {
      const target = change.merge_into.target_id
      mockStores.splice(mockStores.indexOf(store), 1)
      const moved = JSON.parse(JSON.stringify(mockTransactions).replaceAll(store.id, target)) as Transaction[]
      mockTransactions.splice(0, mockTransactions.length, ...moved)
    }
    return resolve(plan)
  },
  update_store: ({ body }) => {
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
        : {
            provider: input.provider,
            import_interval_sec: 900,
            terminal: mockTerminal(),
            import: mockImportSettings(),
          }
    }
    return resolve(store)
  },
  archive_store: ({ args }) => {
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
  },
  delete_store: ({ args }) => {
    const idx = mockStores.findIndex((candidate) => candidate.id === String(args?.id))
    if (idx >= 0) mockStores.splice(idx, 1)
    return resolve({})
  },
}
