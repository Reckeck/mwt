import type { Ref } from 'vue'

import type { UiAction } from '@/shared/ui/context-menu'

/** The «⋯» / right-click menu of a transaction row and the dialogs it opens. */
export interface TxRowMenu {
  /** Menu items for one row: open, edit, duplicate, copy id, delete. */
  actions: (tx: Transaction) => UiAction[]
  editing: Ref<Transaction | null>
  duplicating: Ref<Transaction | null>
  deleting: Ref<Transaction | null>
  /** Wallet the list belongs to: an edit opens from its side. */
  storeId: Ref<string | null>
}

export interface TxRowMenuOptions {
  /** Wallet whose history the list shows. */
  storeId?: () => string | null
  /** Offer «Duplicate» (the full list does; a calendar day does not need it). */
  duplicate?: boolean
}
