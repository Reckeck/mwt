import type { LocationQuery, LocationQueryRaw } from 'vue-router'

/**
 * The operations filter as the address carries it: `?chain=&token=&protocol=&kind=&address=&hidden=1&account=`.
 * A link from another screen (spec E15) opens the list already narrowed, and the page keeps what the user
 * set across a reload and the system «Back». `op` is the open card and is not part of the filter.
 */
export const CRYPTO_OP_QUERY_KEYS = ['chain', 'token', 'protocol', 'kind', 'address', 'hidden', 'account'] as const

const text = (value: unknown): string | undefined => {
  const first = Array.isArray(value) ? value[0] : value
  return typeof first === 'string' && first.trim() ? first.trim() : undefined
}

/** Read the filter out of a route's query; `accountIds` are the accounts the screen fixes (a wallet's own). */
export function cryptoOpFilterFromQuery(query: LocationQuery, accountIds: string[] = []): CryptoOpFilter {
  const asked = (Array.isArray(query.account) ? query.account : [query.account]).filter(
    (value): value is string => typeof value === 'string' && value.length > 0,
  )
  return {
    account_ids: accountIds.length ? accountIds : asked,
    chain: text(query.chain) as Chain | undefined,
    token: text(query.token),
    protocol: text(query.protocol),
    kind: text(query.kind) as CryptoTxKind | undefined,
    address: text(query.address),
    show_hidden: text(query.hidden) === '1',
  }
}

/** The query that says the same as the filter; empty fields are left out, so the address stays short. */
export function cryptoOpFilterToQuery(filter: CryptoOpFilter, fixedAccounts = false): LocationQueryRaw {
  return {
    chain: filter.chain || undefined,
    token: filter.token || undefined,
    protocol: filter.protocol || undefined,
    kind: filter.kind || undefined,
    address: filter.address || undefined,
    hidden: filter.show_hidden ? '1' : undefined,
    account: !fixedAccounts && filter.account_ids.length ? filter.account_ids : undefined,
  }
}

/** How many of the narrowing fields are set (the `Filters` chip). */
export function cryptoOpFilterCount(filter: CryptoOpFilter): number {
  return [filter.chain, filter.token, filter.protocol, filter.kind, filter.address, filter.show_hidden].filter(Boolean)
    .length
}
