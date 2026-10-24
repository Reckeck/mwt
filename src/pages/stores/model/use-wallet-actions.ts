import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { Archive, ArchiveRestore, ExternalLink, Star, StarOff, Trash2 } from 'lucide-vue-next'

import { usePins } from '@/shared/api/pins'
import type { UiAction } from '@/shared/ui/context-menu'

/** The «⋯» of a wallet row, the same on a desktop and a phone; the page confirms a delete. */
export function useWalletActions(onArchive: (store: Store) => void, onDelete: (store: Store) => void) {
  const { t } = useI18n()
  const router = useRouter()
  const pins = usePins()

  const pinned = (store: Store) => pins.isPinned({ kind: 'store', id: store.id })
  const togglePin = (store: Store) => pins.toggle({ kind: 'store', id: store.id })
  /** The backend refuses to delete a wallet with history or sub-accounts; the menu says why. */
  const blocker = (node: StoreTreeNode) => {
    if (node.children.length) return t('stores.deleteHasChildren')
    if (node.own_tx_count > 0) return t('stores.deleteHasTransactions')
    return ''
  }

  const actions = (node: StoreTreeNode): UiAction[] => [
    {
      id: 'open',
      label: t('stores.open'),
      icon: ExternalLink,
      run: () => void router.push({ name: 'wallet', params: { id: node.store.id } }),
    },
    {
      id: 'pin',
      label: pinned(node.store) ? t('stores.unpin') : t('stores.pin'),
      icon: pinned(node.store) ? StarOff : Star,
      run: () => togglePin(node.store),
    },
    {
      id: 'archive',
      label: node.store.archived ? t('stores.unarchive') : t('stores.archive'),
      icon: node.store.archived ? ArchiveRestore : Archive,
      run: () => onArchive(node.store),
    },
    {
      id: 'delete',
      label: t('stores.delete'),
      icon: Trash2,
      destructive: true,
      separated: true,
      disabled: blocker(node) || false,
      run: () => onDelete(node.store),
    },
  ]

  return { actions, pinned, togglePin }
}
