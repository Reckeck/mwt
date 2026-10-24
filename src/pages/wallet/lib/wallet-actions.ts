/** One button of a wallet's header: what it records and how it looks. */
export interface WalletAction {
  kind: TxKind
  label: string
  /** Money moves are outline, exchange trades secondary (Wallets-1 · 7). */
  variant: 'outline' | 'secondary'
  /** A phone keeps it in the grid under the sum. */
  phone: boolean
  /** A divider stands before it: the exchange's trades after the money moves. */
  separated?: boolean
}

const move = (kind: TxKind, label: string, phone = true): WalletAction => ({ kind, label, variant: 'outline', phone })
const trade = (kind: TxKind, label: string, separated = false): WalletAction => ({
  kind,
  label,
  variant: 'secondary',
  phone: true,
  separated,
})

/**
 * The buttons a wallet's kind allows (Wallets-1 · 5–7): a card pays, tops up,
 * converts and transfers; a coin wallet only transfers and converts; an
 * exchange adds Спот · Фьючерс · DeFi, which exist only there. A property has
 * none yet: its bills come with its utilities.
 */
export function walletActions(store: Store, exchangeRoot: boolean): WalletAction[] {
  if (exchangeRoot)
    return [
      move('transfer', 'deal.transfer', false),
      move('convert', 'deal.convert', false),
      trade('trade_spot', 'deal.spot', true),
      trade('trade_futures', 'deal.futures'),
      trade('defi', 'deal.defi'),
    ]
  if (store.kind === 'crypto' || store.kind === 'exchange')
    return [move('transfer', 'deal.transfer'), move('convert', 'deal.convert')]
  if (store.kind === 'property' || store.kind === 'loyalty') return []
  return [
    move('payment', 'wallet.action.payment'),
    move('convert', 'deal.convert'),
    move('transfer', 'deal.transfer'),
    move('topup', 'wallet.action.topup'),
  ]
}
