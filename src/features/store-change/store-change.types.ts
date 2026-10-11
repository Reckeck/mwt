export interface StoreChangeDialogProps {
  store: Store
  /** The action the dialog opens on; the first one allowed by default. */
  initial?: StoreChangeAction | null
}

/** One of the four changes, as the dialog's tabs name them. */
export type StoreChangeAction = 'change_kind' | 'move_to' | 'detach' | 'merge_into'
