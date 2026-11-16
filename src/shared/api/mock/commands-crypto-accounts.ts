/**
 * Commands: the addresses and exchanges the module follows.
 *
 * Owner: spec E9. The browser has no network, so a connected address starts with a demo
 * quarantine (two tokens that "came by themselves") and holdings (`crypto-engine-holdings.ts`, read by
 * `tree.ts`) to exercise the screen. Like the
 * application, a string that looks like a seed phrase or a private key is refused before anything
 * is stored, and its text is never echoed.
 */

import { SECRET_REFUSED, looksLikeSecret } from '@/shared/lib/crypto-secret'

import { type MockCommands, resolve } from './core'
import { mockStores, newStore } from './demo'
import { mockPlacementError } from './tree'
import { mockCryptoTokens } from './commands-crypto-tokens'

interface MockAccount {
  view: CryptoAccountView
  /** What it holds of each quarantined token. */
  held: QuarantinedHolding[]
}
const accounts: MockAccount[] = []

const invalid = (message: string) => Promise.reject({ code: 'INVALID_INPUT', message })
const EVM_CHAINS: Chain[] = ['ethereum', 'arbitrum', 'optimism', 'base', 'polygon', 'bsc']

const DEMO_TOKENS = [
  {
    symbol: 'PEPE2',
    name: 'Pepe 2',
    contract: '0x9a1c4e7b05df2a61c83d9b0e7f114ad2c6e04f02',
    quantity: '1200000',
    days: 28,
    warnings: [] as TokenWarning[],
  },
  {
    symbol: 'USDT-X',
    name: 'USDT-X',
    contract: '0x3bf7c0de5a1b6e2d9f4488a1c7e0b5d3f2aa9a91',
    quantity: '500',
    days: 10,
    warnings: ['imitates_known'] as TokenWarning[],
  },
]

function seedQuarantine(chain: Chain): QuarantinedHolding[] {
  return DEMO_TOKENS.map((demo) => {
    if (!mockCryptoTokens.some((token) => token.contract === demo.contract)) {
      mockCryptoTokens.push({
        chain,
        contract: demo.contract,
        symbol: demo.symbol,
        name: demo.name,
        decimals: 18,
        status: 'quarantined',
        basis_usd: null,
        warnings: demo.warnings,
      })
    }
    return {
      chain,
      contract: demo.contract,
      quantity: demo.quantity,
      first_seen: new Date(Date.now() - demo.days * 86_400_000).toISOString(),
    }
  })
}

/** A demo month for the screen: the mock has no snapshots, so a connected address pretends to have a month of them. */
const DEMO_MONTH: Pick<MonthChange, 'change_usd' | 'change_pct'> = { change_usd: '312.40', change_pct: '3.6' }

const viewOf = (entry: MockAccount): CryptoAccountView => ({
  ...entry.view,
  month_change: { ...DEMO_MONTH, since: new Date(Date.now() - 30 * 86_400_000).toISOString() },
  quarantine: entry.held.filter((holding) =>
    mockCryptoTokens.some((token) => token.contract === holding.contract && token.status === 'quarantined'),
  ),
})

/**
 * The ids of the addresses being followed, for the tracking status (`commands-crypto-sync.ts`): in
 * the browser every followed address already holds what `crypto-engine-holdings.ts` gives it, so the
 * status must report a finished read — a wallet with assets that was "never read" cannot happen in
 * the application.
 */
export function mockFollowedAccountIds(): string[] {
  return accounts.map((item) => item.view.id)
}

export const cryptoAccountsCommands: MockCommands = {
  crypto_list_accounts: () => resolve(accounts.map(viewOf)),
  crypto_connect_address: ({ body }) => {
    const input = body as unknown as ConnectAddressInput
    // First of all, and the text is never repeated back.
    if (looksLikeSecret(input.address)) return invalid(SECRET_REFUSED)
    if (!EVM_CHAINS.includes(input.chain)) return invalid('crypto: this network cannot be followed yet')
    const address = input.address.trim().toLowerCase()
    if (!/^0x[0-9a-f]{40}$/.test(address))
      return invalid('crypto: a public address starts with 0x and has 40 hex digits')
    if (accounts.some((entry) => entry.view.chain === input.chain && entry.view.address === address)) {
      return Promise.reject({ code: 'ALREADY_EXISTS', message: 'this address is already followed' })
    }
    const name = input.name.trim() || `${address.slice(0, 6)}…${address.slice(-4)}`
    const failure = mockPlacementError(null, null, name)
    if (failure) return Promise.reject(failure)
    const id = `cacc-${crypto.randomUUID()}`
    const store: Store = { ...newStore(name, 'crypto', '#627eea'), custody: 'hot', crypto_account_id: id }
    mockStores.push(store)
    accounts.push({
      view: {
        id,
        name,
        chain: input.chain,
        address,
        source: null,
        archived: false,
        note: '',
        created_at: input.start_date ? `${input.start_date}T00:00:00Z` : '1970-01-01T00:00:00Z',
        store_id: store.id,
        quarantine: [],
      },
      held: seedQuarantine(input.chain),
    })
    return resolve({ account_id: id, store_id: store.id } satisfies ConnectAddressResult)
  },
  crypto_update_account: ({ body }) => {
    const input = body as unknown as UpdateAccountInput
    const entry = accounts.find((candidate) => candidate.view.id === input.account_id)
    if (!entry) return Promise.reject({ code: 'NOT_FOUND', message: 'crypto account' })
    if (input.name !== undefined) {
      if (!input.name.trim()) return invalid('crypto: the name must not be empty')
      entry.view.name = input.name.trim()
      const store = mockStores.find((candidate) => candidate.id === entry.view.store_id)
      if (store) store.name = entry.view.name
    }
    if (input.note !== undefined) entry.view.note = input.note.trim()
    if (input.archived !== undefined) entry.view.archived = input.archived
    return resolve(null)
  },
  crypto_forget_account: ({ body }) => {
    const input = body as unknown as CryptoAccountRefInput
    const index = accounts.findIndex((candidate) => candidate.view.id === input.account_id)
    if (index < 0) return Promise.reject({ code: 'NOT_FOUND', message: 'crypto account' })
    const [gone] = accounts.splice(index, 1)
    const store = mockStores.find((candidate) => candidate.id === gone?.view.store_id)
    if (store) {
      store.crypto_account_id = undefined
      store.archived = true
    }
    return resolve(null)
  },
}
