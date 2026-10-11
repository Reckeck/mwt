export interface TransactionFormProps {
  /** Wallet the form is opened on; it fills the store fields. */
  storeId: string
  /** A stored transaction to edit; `null` creates a new one. */
  transaction?: Transaction | null
  /** Calculator plan to prefill once; a new object applies again. */
  plan?: TransactionPlan | null
  disabled?: boolean
  /** Opens a transfer to this side (a contact's card: «write a transfer here»). */
  counterpart?: AccountRef | null
  /** With `counterpart`: a debt operation — lending and repaying send to it, borrowing and collecting come from it. */
  debt?: DebtEffect | null
  /**
   * Edit a template for an automation instead of a transaction: only the four
   * verbs, no date, price or on-chain fields, and nothing is saved — the
   * parent reads `input` and submits it itself. `transaction` prefills it.
   */
  template?: boolean
  /** Continue this chain: the new operation joins it (plan §6.4). */
  flowId?: string | null
  /** A new operation pre-filled from this one (a utility bill); saved as new. */
  prefill?: Transaction | null
  /** A new operation starting as this kind with these categories (an automation preset). */
  initial?: TransactionInitial | null
}

export interface TransactionInitial {
  kind: TxKind
  categories: string[]
}

/** A calculator result handed to the form. */
export interface TransactionPlan {
  mode: CalcMode
  from: AssetRef | null
  to: AssetRef | null
  amount: string
  /** USD price of `from`. */
  price: string
}

export interface TransactionCreateDialogProps {
  storeId: string
  /** Archived wallets take no new transactions. */
  disabled?: boolean
  /** Calculator plan to prefill. */
  plan?: TransactionPlan | null
  /** Opens a transfer to this side. */
  counterpart?: AccountRef | null
  /** With `counterpart`: which debt operation the transfer is. */
  debt?: DebtEffect | null
  /** Continue this chain. */
  flowId?: string | null
  /** A new operation pre-filled from this one. */
  prefill?: Transaction | null
}

export interface TransactionEditDialogProps {
  transaction: Transaction
  /** Wallet the edit was opened from; defaults to the transaction's own. */
  storeId?: string
}

/** How an exchange goes: straight between accounts, or through someone (P2P). */
export type ConvertMode = 'direct' | 'p2p'

/**
 * One more recipient of an exchange or a top-up. Each becomes its own
 * operation; together with the main one they are saved as one chain.
 */
export interface SplitRow {
  id: string
  /** P2P: who the money goes through. */
  via: AccountRef | null
  /** Where it arrives. */
  to: AccountRef | null
  /** Exchange: what arrives there. */
  asset: AssetRef | null
  /** Exchange: what this recipient gets of the given asset. */
  give: string
  /** What arrives: the exchange's received amount, the top-up's amount. */
  receive: string
}
