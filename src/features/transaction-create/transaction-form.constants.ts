/** The short form's tabs, in the canvas order (OpForm-1 · 1); trades and DeFi are read by the crypto module. */
export const QUICK_KINDS: TxKind[] = ['payment', 'convert', 'transfer', 'topup']
/** The exchange's own wallet (OpForm-2): moving money only; what happens on the exchange is read, not typed. */
export const DEAL_GROUPS: TxKind[][] = [['convert', 'transfer']]
