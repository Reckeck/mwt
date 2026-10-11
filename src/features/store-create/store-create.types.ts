export interface StoreCreateDialogProps {
  /**
   * Parent wallet: the dialog then creates a typed sub-account of it.
   * Without it the dialog creates a top-level wallet.
   */
  parent?: Store | null
  /** Names from the top-level wallet down to `parent`, for the title. */
  parentPath?: string[]
  /** Pre-selected kind for a top-level wallet (the sidebar group's «+»): skips the first step. */
  kind?: StoreKind | null
}
