import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { Eye, Pencil, Repeat2, Search, Tag, Trash2, User, Wallet } from 'lucide-vue-next'

import type { UiAction } from '@/shared/ui/context-menu'
import { openEntity } from '@/shared/lib/entity-link'
import { accountContactId, similarFilter, similarLocation, txMeta, txSides, txStoreId } from '@/shared/lib/transactions'
import { openTxDetails } from '@/shared/lib/tx-details'
import { useTxContext } from '@/shared/lib/tx-context'

import type { TxRowMenu, TxRowMenuOptions } from './transaction-row-menu.types'

/**
 * One menu for every transaction list (Paths 2 · 11): the glance and the edit
 * first, then what the operation leads to — alike ones, its wallet, its
 * contact — and the delete last, in red, below a line. Right click, «⋯» and a
 * long press on a phone open the same list. The dialogs live in
 * `TxRowMenuHost`, mounted once next to the list.
 */
export function useTxRowMenu(options: TxRowMenuOptions = {}): TxRowMenu {
  const { t } = useI18n()
  const router = useRouter()
  const context = useTxContext()
  const editing = ref<Transaction | null>(null)
  const duplicating = ref<Transaction | null>(null)
  const deleting = ref<Transaction | null>(null)
  const storeId = computed(() => options.storeId?.() ?? null)

  function similar(tx: Transaction) {
    const to = similarLocation(tx)
    if (to) void router.push(to)
  }

  function actions(tx: Transaction): UiAction[] {
    const wallet = context.rootOf(txStoreId(tx))
    const sides = txSides(tx)
    const contact = txMeta(tx).contact_id ?? accountContactId(sides.to) ?? accountContactId(sides.from)
    const list: UiAction[] = [
      {
        id: 'glance',
        label: t('tx.menu.glance'),
        icon: Eye,
        shortcut: t('tx.menu.key.space'),
        run: () => openTxDetails(tx),
      },
      { id: 'edit', label: t('tx.menu.edit'), icon: Pencil, shortcut: 'Enter', run: () => (editing.value = tx) },
    ]
    if (options.duplicate) {
      list.push({
        id: 'duplicate',
        label: t('tx.menu.repeat'),
        icon: Repeat2,
        shortcut: 'Ctrl D',
        run: () => (duplicating.value = tx),
      })
    }
    const related: UiAction[] = [
      {
        id: 'similar',
        label: t('tx.menu.similar'),
        icon: Search,
        hidden: !similarFilter(tx),
        run: () => similar(tx),
      },
      {
        id: 'wallet',
        label: t('tx.menu.wallet', { name: context.storeName(wallet) }),
        icon: Wallet,
        hidden: !wallet || wallet === (storeId.value && context.rootOf(storeId.value)),
        run: () => openEntity({ kind: 'store', id: wallet }),
      },
      ...txMeta(tx).categories.map((key) => ({
        id: `category:${key}`,
        label: t('tx.menu.category', { name: context.categoryName(key) }),
        icon: Tag,
        run: () => openEntity({ kind: 'category', id: key }),
      })),
      {
        id: 'contact',
        label: t('tx.menu.contact', { name: contact ? context.contactName(contact) : '' }),
        icon: User,
        hidden: !contact,
        run: () => contact && openEntity({ kind: 'contact', id: contact }),
      },
    ]
    // The line goes above whichever of them this operation has.
    const first = related.find((action) => !action.hidden)
    if (first) first.separated = true
    list.push(...related, {
      id: 'delete',
      label: t('tx.menu.delete'),
      icon: Trash2,
      shortcut: 'Del',
      destructive: true,
      separated: true,
      run: () => (deleting.value = tx),
    })
    return list
  }

  return { actions, editing, duplicating, deleting, storeId }
}
