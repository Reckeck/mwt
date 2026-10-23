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
  /** The property the operation starts linked to (an automation's link, a preset opened on a property). */
  propertyId?: string | null
  /**
   * The short form of Ctrl N (OpForm-1): four tabs, the amount with its
   * currency chip, the wallet (`#store`) next to the category, the rest under
   * «Ещё»; no balance preview.
   */
  compact?: boolean
  /** With `compact`: the tab groups of «Сделка · <биржа>» instead of the four quick kinds. */
  kindGroups?: TxKind[][] | null
}

export interface TransactionInitial {
  kind: TxKind
  categories: string[]
  /** What was typed before the wallet changed: the short form keys itself by the wallet and carries these over. */
  amount?: string
  comment?: string
}

/** A calculator result handed to the form. */
export interface TransactionPlan {
  mode: CalcMode
  from: AssetRef | null
  to: AssetRef | null
  amount: string
  /** `position`: units of `to` the amount bought. */
  quantity?: string
  /** USD price of `from`; empty leaves the price to the form. */
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
 * One more account the money lands on (the first one is the form's own
 * «to»). Each becomes its own operation; all are saved as one chain.
 */
export interface SplitRow {
  id: string
  /** Where it arrives. */
  to: AccountRef | null
  /** How much arrives there. */
  receive: string
}
