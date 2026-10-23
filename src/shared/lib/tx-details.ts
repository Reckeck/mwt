import { ref } from 'vue'

import { openEntity } from './entity-link'
import { txId } from './transactions'

/**
 * The transaction whose card is open. One card for the whole app: the shell
 * draws it at `?open=tx:<id>`, and any list (a wallet, a contact, a flow, an
 * import) opens it with `openTxDetails` instead of carrying its own copy of
 * the sheet. The list's copy is kept so the card shows at once.
 */
export const txDetailsTx = ref<Transaction | null>(null)

/** The rows the card was opened from: its ‹ › step through them. */
export const txDetailsList = ref<Transaction[]>([])

export function openTxDetails(tx: Transaction, list?: Transaction[]) {
  txDetailsTx.value = tx
  if (list) txDetailsList.value = list
  else if (!txDetailsList.value.some((row) => txId(row) === txId(tx))) txDetailsList.value = []
  openEntity({ kind: 'tx', id: txId(tx) })
}
