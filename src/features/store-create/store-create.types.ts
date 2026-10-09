export interface StoreCreateDialogProps {
  /**
   * Parent node: the dialog then creates a sub-account of it and starts from
   * its kind and colour. Without it the dialog creates a top-level wallet.
   */
  parent?: Store | null
  /** Names from the top-level wallet down to `parent`, for the title. */
  parentPath?: string[]
}
