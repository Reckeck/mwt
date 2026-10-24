import { ref } from 'vue'

/**
 * Whether the «+ Операция» dialog is open. One flag for the whole app: the
 * shell draws the dialog, and any widget (the checklist, an empty state) may
 * open it without reaching up into the shell.
 */
export const quickAddOpen = ref(false)

/** What a caller asked the dialog to start on: «Перевести» from a wallet's glance. */
export interface QuickAddRequest {
  storeId?: string
  kind?: TxKind
  /** A payment's category: «Записать оплату в „Продукты“» (Paths 2 · 8). */
  category?: string
  /** A transfer's destination: «Снять с карты» on an empty wallet (Wallets-3 · 18). */
  toStoreId?: string
  /** The sum to start on: «Записать обмен 100 USD» from the Ctrl K converter (Currency-2 · 10). */
  amount?: string
}

/** The request of the current opening; the dialog clears it on close. */
export const quickAddRequest = ref<QuickAddRequest | null>(null)

/** Opens «+ Операция» on a wallet and a kind. */
export function openQuickAdd(request: QuickAddRequest = {}) {
  quickAddRequest.value = request
  quickAddOpen.value = true
}
