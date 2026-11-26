import type { RouteRecordRaw } from 'vue-router'

/**
 * The crypto module's own screens (stage 3). The operations list lives under
 * «Transactions» (spec E10), because it is the same kind of thing as a
 * transaction; a DeFi position is not, so it has a section of its own.
 */
export const cryptoRoutes: RouteRecordRaw = {
  path: 'crypto',
  children: [
    {
      // What the followed addresses hold inside protocols (spec D0; the list itself is D6).
      path: 'positions',
      name: 'crypto-positions',
      component: () => import('@/pages/crypto-positions').then((page) => page.CryptoPositionsPage),
      meta: { i18n: 'cryptoTrack.positions.title' },
    },
  ],
}
