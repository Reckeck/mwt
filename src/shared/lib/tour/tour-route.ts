import type { RouteLocationRaw } from 'vue-router'

/** Routes that show one wallet: a tour there needs a wallet to show. */
const WALLET_ROUTES = new Set(['wallet', 'wallet-settings'])

/**
 * Where to go to see a route's tour: the route, with `?tour=` so the shell
 * starts it there; a wallet route opens `walletId` (the first wallet).
 */
export function tourLocation(route: string, walletId: string | null): RouteLocationRaw | null {
  if (WALLET_ROUTES.has(route)) {
    return walletId ? { name: route, params: { id: walletId }, query: { tour: route } } : null
  }
  return { name: route, query: { tour: route } }
}
