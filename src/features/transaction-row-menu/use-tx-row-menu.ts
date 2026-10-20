import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Copy, CopyPlus, ExternalLink, Pencil, Trash2 } from 'lucide-vue-next'

import type { UiAction } from '@/shared/ui/context-menu'
import { copyText } from '@/shared/lib/clipboard'
import { txId } from '@/shared/lib/transactions'
import { openTxDetails } from '@/shared/lib/tx-details'

import type { TxRowMenu, TxRowMenuOptions } from './transaction-row-menu.types'

/**
 * One menu for every transaction list: the row keeps a single «⋯» in a column
 * of fixed width, so hovering a row moves nothing (rule 17). The dialogs live
 * in `TxRowMenuHost`, mounted once next to the list.
 */
export function useTxRowMenu(options: TxRowMenuOptions = {}): TxRowMenu {
  const { t } = useI18n()
  const editing = ref<Transaction | null>(null)
  const duplicating = ref<Transaction | null>(null)
  const deleting = ref<Transaction | null>(null)
  const storeId = computed(() => options.storeId?.() ?? null)

  function actions(tx: Transaction): UiAction[] {
    const list: UiAction[] = [
      { id: 'open', label: t('tx.openDetails'), icon: ExternalLink, run: () => openTxDetails(tx) },
      { id: 'edit', label: t('tx.edit'), icon: Pencil, run: () => (editing.value = tx) },
    ]
    if (options.duplicate) {
      list.push({ id: 'duplicate', label: t('tx.duplicate'), icon: CopyPlus, run: () => (duplicating.value = tx) })
    }
    list.push(
      { id: 'copy', label: t('tx.copyId'), icon: Copy, separated: true, run: () => void copyText(txId(tx)) },
      {
        id: 'delete',
        label: t('tx.deleteMenu'),
        icon: Trash2,
        destructive: true,
        separated: true,
        run: () => (deleting.value = tx),
      },
    )
    return list
  }

  return { actions, editing, duplicating, deleting, storeId }
}
