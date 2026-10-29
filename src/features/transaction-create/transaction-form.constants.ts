/** The short form's tabs, in the canvas order (OpForm-1 · 1); trades open from the exchange (OpForm-2 · 9). */
export const QUICK_KINDS: TxKind[] = ['payment', 'convert', 'transfer', 'topup']
/** `Trade · <exchange>` (OpForm-2 · 9): moving money first, then what only an exchange does. */
export const DEAL_GROUPS: TxKind[][] = [
  ['convert', 'transfer'],
  ['trade_spot', 'trade_futures', 'defi'],
]
/** The phone's deal sheet keeps only the exchange's own kinds. */
export const DEAL_PHONE_GROUPS: TxKind[][] = [['trade_spot', 'trade_futures', 'defi']]
/** DeFi actions the short form offers; unstaking and claims stay in the full editor. */
export const QUICK_DEFI_ACTIONS: DefiAction[] = ['stake', 'add_liquidity']
