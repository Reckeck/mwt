import type { RouteLocationRaw } from 'vue-router'

/** Routes that show one wallet: a tour there needs a wallet to show. */
const WALLET_ROUTES = new Set(['wallet', 'wallet-settings'])
/** A What's-new point may open a settings tab instead of a screen. */
const SETTINGS_PREFIX = 'settings:'

/**
 * Where to go to see a route's tour: the route, with `?tour=` so the shell
 * starts it there; a wallet route opens `walletId` (the first wallet).
 */
export function tourLocation(route: string, walletId: string | null): RouteLocationRaw | null {
  // `settings:<tab>` opens that tab of the settings window over the current screen.
  if (route.startsWith(SETTINGS_PREFIX)) return { query: { settings: route.slice(SETTINGS_PREFIX.length) } }
  if (WALLET_ROUTES.has(route)) {
    return walletId ? { name: route, params: { id: walletId }, query: { tour: route } } : null
  }
  return { name: route, query: { tour: route } }
}
