/**
 * Commands: entering the module and the archive of what was kept by hand.
 *
 * Owner: spec E11, with `service/cryptotrack/transition.rs`. The mock applies the same rule as the
 * backend — a wallet of kind `crypto` or `exchange` with records in it is retired unless the module
 * reads it: a followed address, or an exchange of a provider the module talks to (its key can only
 * be stored at level 3, so it arrives after the move and says nothing here) — and archives in
 * place, deleting nothing, so a second call reports `changed: false` as the vault does.
 *
 * The mock has no holdings, so what a wallet takes out of the totals is a demo number per shape
 * (the `crypto` one is the 412 300 of the layout); everything else is read from the mock's state.
 */

import { resolve } from './core'
import type { MockCommands } from './core'
import { mockFilterTxs, mockStores } from './demo'

/** What a wallet of this shape pretends to hold, since the mock has no holdings. */
const DEMO_LEAVING_BASE: Record<'crypto' | 'exchange' | 'sub', string> = {
  crypto: '412300',
  exchange: '150000',
  sub: '12300',
}

const rootOf = (store: Store): Store => {
  let current = store
  while (current.parent_id) {
    const parent = mockStores.find((have) => have.id === current.parent_id)
    if (!parent) break
    current = parent
  }
  return current
}

/** Whether the module reads this wallet instead of the user keeping it by hand. */
const engineReads = (store: Store): boolean => {
  const root = rootOf(store)
  if (store.crypto_account_id || root.crypto_account_id) return true
  return root.kind === 'exchange' && root.exchange?.provider === 'bybit'
}

const isCrypto = (store: Store) => store.kind === 'crypto' || store.kind === 'exchange'
const hasRecords = (store: Store) => mockFilterTxs([store.id]).length > 0

/** The families the move retires: records anywhere under a wallet retire the whole family. */
const retired = (): Store[] => {
  const families = new Set(
    mockStores
      .filter((store) => !store.archived && isCrypto(store) && hasRecords(store) && !engineReads(store))
      .map((store) => rootOf(store).id),
  )
  return mockStores.filter((store) => !store.archived && isCrypto(store) && families.has(rootOf(store).id))
}

const leavingBase = (store: Store): string =>
  store.parent_id ? DEMO_LEAVING_BASE.sub : DEMO_LEAVING_BASE[store.kind === 'crypto' ? 'crypto' : 'exchange']

const retiredView = (store: Store): CryptoRetiredWallet => ({
  store_id: store.id,
  name: store.name,
  kind: store.kind,
  leaving_base: leavingBase(store),
})

const sum = (wallets: CryptoRetiredWallet[]): string =>
  String(wallets.reduce((total, wallet) => total + Number(wallet.leaving_base), 0))

export const cryptoTransitionCommands: MockCommands = {
  crypto_transition_plan: () => {
    const wallets = retired().map(retiredView)
    return resolve({ base_currency: 'rub', wallets, leaving_base: sum(wallets) } satisfies CryptoTransitionPlan)
  },
  crypto_archive_manual_wallets: () => {
    const wallets = retired()
    const view = wallets.map(retiredView)
    for (const store of wallets) store.archived = true
    return resolve({ changed: wallets.length > 0, wallets: view } satisfies CryptoArchiveReport)
  },
}
