/** One button of a wallet's header: what it records and how it looks. */
export interface WalletAction {
  kind: TxKind
  label: string
  /** A phone keeps it in the grid under the sum. */
  phone: boolean
}

const move = (kind: TxKind, label: string, phone = true): WalletAction => ({ kind, label, phone })

/**
 * The buttons a wallet's kind allows (Wallets-1 · 5–7): a card pays, tops up,
 * converts and transfers; a coin wallet and an exchange only transfer and
 * convert (what happens on-chain or on an exchange is read by the crypto
 * module, nothing is typed by hand). A property has none yet: its bills come
 * with its utilities.
 */
export function walletActions(store: Store, exchangeRoot: boolean): WalletAction[] {
  if (exchangeRoot || store.kind === 'crypto' || store.kind === 'exchange')
    return [move('transfer', 'deal.transfer'), move('convert', 'deal.convert')]
  if (store.kind === 'property' || store.kind === 'loyalty') return []
  return [
    move('payment', 'wallet.action.payment'),
    move('convert', 'deal.convert'),
    move('transfer', 'deal.transfer'),
    move('topup', 'wallet.action.topup'),
  ]
}
