import type { TourStep } from './tours.types.ts'

/**
 * Connected addresses (spec E9). The button lives on the wallets list; the rest is on the wallet page of
 * an address, where a step whose element is not on screen is skipped (a wallet of another kind has none of them).
 */
export const CRYPTO_STORES_STEPS: TourStep[] = [{ target: 'crypto-add-address', key: 'stores.cryptoAdd', side: 'top' }]

export const CRYPTO_WALLET_STEPS: TourStep[] = [
  { target: 'crypto-overview', key: 'wallet.cryptoOverview', side: 'bottom' },
  { target: 'crypto-assets', key: 'wallet.cryptoAssets', side: 'top' },
  { target: 'crypto-quarantine', key: 'wallet.cryptoQuarantine', side: 'top' },
  { target: 'crypto-relations', key: 'wallet.cryptoRelations', side: 'top' },
]

/** The operations screen (spec E10). The card steps exist only while an operation is open (`?op=`); otherwise they are skipped. */
export const CRYPTO_TOURS: Record<string, TourStep[]> = {
  'crypto-operations': [
    { target: 'crypto-operations', key: 'cryptoOperations.list', side: 'top' },
    { target: 'crypto-tx-override', key: 'cryptoOperations.override', side: 'left' },
    { target: 'crypto-tx-link', key: 'cryptoOperations.link', side: 'left' },
    { target: 'crypto-pair', key: 'cryptoOperations.pair', side: 'left' },
  ],
  'crypto-positions': [
    { target: 'defi-positions', key: 'defi.list', side: 'top' },
    { target: 'defi-position-net', key: 'defi.net', side: 'bottom' },
    { target: 'defi-fees', key: 'defi.fees', side: 'bottom' },
  ],
}

/**
 * `data-tour` anchors of the crypto module that no route tour can reach, and why (the `check-onboarding` gate reads this).
 * The address form lives in a dialog, which a route tour cannot open; it is explained by the dialog's own description and field hints.
 * The detail dialog of a DeFi position (spec D6) is the same: its breakdown card and its collected-fees line carry their own
 * names, so they never compete with the net tile and the fees column on the page.
 */
export const CRYPTO_TOURLESS_TARGETS: Record<string, string> = {
  'crypto-add-address-form':
    'Inside the connect-an-address dialog: a route tour cannot reach it; the dialog description and the field hints explain it.',
  'defi-position-breakdown':
    'Inside the position detail dialog: a route tour cannot open it; the dialog description and the breakdown note explain it.',
}
