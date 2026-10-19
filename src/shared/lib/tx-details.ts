import { ref } from 'vue'

/**
 * The transaction whose card is open. One card for the whole app: the shell
 * draws it, and any list (a wallet, a contact, a flow, an import) opens it
 * with `openTxDetails` instead of carrying its own copy of the sheet.
 */
export const txDetailsOpen = ref(false)
export const txDetailsTx = ref<Transaction | null>(null)

export function openTxDetails(tx: Transaction) {
  txDetailsTx.value = tx
  txDetailsOpen.value = true
}
