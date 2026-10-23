/** Commands: contacts and transactions. */

import { txContactIds, txId, txLegs, txMeta } from '@/shared/lib/transactions'
import { mockContactBalances, mockContacts, mockKindsFor, mockTransactionFrom, mockTurnover } from './ledger'
import { mockDebtTerms } from './catalog'
import { emitMock } from '../mock-bus'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockBalanceOf, mockPush, mockTransactions, priceOf } from './demo'
import { mockCachedPrice, mockCalc, mockNetWorth, mockSearch } from './market'
import { mockPreview } from './preview'
import { mockLinkProperty, mockPropertyLinks } from './property-links'

/** Commands: contacts. */
export const contactsCommands: MockCommands = {
  list_contacts: (_call) => {
    return resolve(mockContacts)
  },
  create_contact: ({ args }) => {
    const input = args?.input as ContactDraft
    const contact: Contact = {
      id: crypto.randomUUID(),
      name: input.name.trim(),
      kind: input.kind,
      note: input.note,
      group: input.group.trim(),
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
  },
  update_contact: ({ body }) => {
    const input = body as UpdateContactInput
    const contact = mockContacts.find((entry) => entry.id === input.id)
    if (!contact) throw { code: 'MISSING', message: 'contact' }
    Object.assign(contact, {
      name: input.contact.name.trim(),
      kind: input.contact.kind,
      note: input.contact.note,
      group: input.contact.group.trim(),
      accounts: input.contact.accounts.map((account) => ({
        ...account,
        id: account.id || crypto.randomUUID(),
        active: account.id ? account.active : true,
      })),
    })
    return resolve(contact)
  },
  archive_contact: ({ args }) => {
    const contact = mockContacts.find((entry) => entry.id === String(args?.id))
    if (contact) contact.archived = Boolean(args?.archived)
    return resolve({})
  },
  delete_contact: ({ args }) => {
    const id = String(args?.id)
    if (mockTransactions.some((tx) => txContactIds(tx).includes(id)))
      throw { code: 'INVALID_INPUT', message: 'the contact is used by transactions; archive it instead' }
    mockContacts.splice(
      mockContacts.findIndex((entry) => entry.id === id),
      1,
    )
    return resolve({})
  },
  get_contact_turnover: ({ body }) => {
    const input = body as ContactTurnoverInput
    return resolve(mockTurnover(mockTransactions, input.contact_id, input.account_id, txLegs))
  },
  list_debt_terms: ({ args }) => {
    const contactId = args?.contactId ? String(args.contactId) : null
    return resolve(mockDebtTerms.filter((entry) => !contactId || entry.contact_id === contactId))
  },
  set_debt_terms: ({ body }) => {
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
  },
  get_contact_balances: (_call) => {
    return resolve(mockContactBalances(mockTransactions))
  },
  get_net_worth: (_call) => {
    return resolve(mockNetWorth())
  },
  set_manual_price: ({ body }) => {
    const input = body as SetManualPriceInput
    return resolve({ ...input, updated_at: new Date().toISOString() })
  },
  clear_manual_price: (_call) => {
    return resolve({})
  },
}

/** Commands: transactions. */
export const transactionsCommands: MockCommands = {
  transaction_kinds_for_store_kind: ({ args }) => {
    return resolve(mockKindsFor(args?.kind as StoreKind))
  },
  add_linked_transactions: ({ body }) => {
    // Like the backend: one chain, all or none (the mock validates each first).
    const input = body as AddLinkedTransactionsInput
    const flow = input.flow_id ?? crypto.randomUUID()
    const built = input.transactions.map((draft) =>
      mockTransactionFrom(
        { ...draft, common: { ...draft.common, flow_id: flow } } as TransactionInput,
        priceOf,
        undefined,
        mockBalanceOf,
      ),
    )
    return resolve(built.map(mockPush))
  },
  add_transaction: ({ args }) => {
    const input = args?.input as TransactionInput
    const tx = mockPush(mockTransactionFrom(input, priceOf, undefined, mockBalanceOf))
    mockLinkProperty(txId(tx), input.common.property_id)
    return resolve(tx)
  },
  tx_property_links: () => resolve([...mockPropertyLinks]),
  get_adjustment_draft: ({ body }) => {
    const input = body as AdjustmentDraftInput
    const totals = new Map<string, { asset: AssetRef; total: number }>()
    for (const tx of mockTransactions) {
      if (txId(tx) === input.replaces) continue
      for (const leg of txLegs(tx)) {
        if (leg.store_id !== input.store_id) continue
        const entry = totals.get(leg.asset.symbol) ?? { asset: leg.asset, total: 0 }
        entry.total += Number(leg.amount)
        totals.set(leg.asset.symbol, entry)
      }
    }
    const lines = [...totals.values()]
      .filter((entry) => entry.total !== 0)
      .map((entry) => ({
        asset: entry.asset,
        before: String(entry.total),
        after: String(entry.total),
        price: { price_usd: null, price_base: null, fx_override: null },
      }))
    return resolve({
      store_id: input.store_id,
      at: input.at || new Date().toISOString(),
      lines,
    } satisfies AdjustmentDraft)
  },
  preview_transaction: ({ body }) => {
    return resolve(mockPreview(body as PreviewTransactionInput))
  },
  update_transaction: ({ body }) => {
    const input = body as UpdateTransactionInput
    const index = mockTransactions.findIndex((tx) => txId(tx) === input.id)
    if (index < 0) throw { code: 'MISSING', message: 'transaction' }
    const kept = { id: input.id, origin: txMeta(mockTransactions[index]).origin }
    mockTransactions[index] = mockTransactionFrom(input.transaction, priceOf, kept, (storeId, symbol) =>
      mockBalanceOf(storeId, symbol, input.id),
    )
    mockLinkProperty(input.id, input.transaction.common.property_id)
    return resolve(mockTransactions[index])
  },
  delete_transaction: ({ args }) => {
    const idx = mockTransactions.findIndex((tx) => txId(tx) === String(args?.id))
    if (idx >= 0) mockTransactions.splice(idx, 1)
    mockLinkProperty(String(args?.id), null)
    return resolve({})
  },
  search_assets: ({ body }) => {
    return resolve(mockSearch(body?.query as string))
  },
  get_cached_price: ({ body }) => {
    return resolve(mockCachedPrice(body as AssetPriceInput))
  },
  convert_amount: ({ body }) => {
    const input = body as ConvertAmountInput
    const from = priceOf(input.from)
    const to = priceOf(input.to)
    return resolve({ amount: from && to ? String((Number(input.amount) * from) / to) : null })
  },
  calc_preview: ({ body }) => {
    return resolve(mockCalc(body as CalcInput))
  },
  get_shortcut_support: (_call) => {
    return resolve({
      global_keys: false,
      launch_command: '/usr/bin/multi-wallet-tracker --quick-add',
    } satisfies ShortcutSupport)
  },
  ensure_fx: (_call) => {
    return resolve({ points: [], missing: [] } satisfies EnsureFxOutput)
  },
  refresh_all: (_call) => {
    const result: RefreshResult = { prices: 3, fx: 1, restamped: 0, updated_at: new Date().toISOString() }
    // Like the backend: the screens follow the event, not the command's answer.
    setTimeout(() => emitMock('prices-updated', result), 0)
    return resolve(result)
  },
  open_explorer: (_call) => {
    return resolve({})
  },
}
