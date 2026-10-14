import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import {
  Archive,
  ArchiveRestore,
  ChevronsDownUp,
  Copy,
  ExternalLink,
  FolderPlus,
  Plus,
  Repeat,
  Settings2,
} from 'lucide-vue-next'

import type { UiAction } from '@/shared/ui/context-menu'
import { call } from '@/shared/api/ipc'
import { copyText } from '@/shared/lib/clipboard'

import { canAddOfKind } from '../lib/totals'

/**
 * Right-click menus of the sidebar: everything one can do with a wallet (or a
 * kind of wallets) without opening it, and the dialogs those items open.
 */
export function useWalletMenu(openCreate: (kind: StoreKind | null) => void, collapseAll: () => void) {
  const { t } = useI18n()
  const router = useRouter()
  const queryClient = useQueryClient()

  /** The wallet a new sub-account goes under; the dialog is open while it is set. */
  const subParent = ref<Store | null>(null)
  const subOpen = computed({
    get: () => subParent.value !== null,
    set: (value: boolean) => {
      if (!value) subParent.value = null
    },
  })
  /** The wallet a new operation is recorded on; the dialog is open while it is set. */
  const txStore = ref('')
  const txOpen = computed({
    get: () => txStore.value !== '',
    set: (value: boolean) => {
      if (!value) txStore.value = ''
    },
  })

  const archive = useMutation({
    mutationFn: ({ id, archived }: { id: string; archived: boolean }) => call('archive_store', { id, archived }),
    onSuccess: () => {
      for (const queryKey of [['stores'], ['store'], ['summary']]) void queryClient.invalidateQueries({ queryKey })
    },
    meta: {
      error: 'wallet.archiveFailed',
      success: (_data, variables) =>
        (variables as { archived: boolean }).archived ? t('wallet.archived') : t('wallet.unarchived'),
      undo: (_data, variables) => {
        const { id, archived } = variables as { id: string; archived: boolean }
        return call('archive_store', { id, archived: !archived })
      },
    },
  })

  function walletActions(node: StoreTreeNode): UiAction[] {
    const store = node.store
    const isCash = store.kind === 'cash' && !store.parent_id
    const settingsTo = (tab: string) => () =>
      void router.push({ name: 'wallet-settings', params: { id: store.id }, query: { tab } })
    return [
      {
        id: 'open',
        label: t('actions.open'),
        icon: ExternalLink,
        run: () => void router.push({ name: 'wallet', params: { id: store.id } }),
      },
      {
        id: 'tx',
        label: t('wallet.addOperation'),
        icon: Plus,
        disabled: store.archived ? t('wallet.archivedHint') : false,
        run: () => (txStore.value = store.id),
      },
      {
        id: 'sub',
        label: t('stores.newSubAccountShort'),
        icon: FolderPlus,
        hidden: Boolean(store.parent_id) || !node.can_add_child,
        run: () => (subParent.value = store),
      },
      { id: 'settings', label: t('walletSettings.nav'), icon: Settings2, separated: true, run: settingsTo('general') },
      { id: 'automations', label: t('walletSettings.automations'), icon: Repeat, run: settingsTo('automations') },
      { id: 'copy', label: t('actions.copyName'), icon: Copy, run: () => void copyText(store.name) },
      {
        id: 'archive',
        label: store.archived ? t('stores.unarchive') : t('stores.archive'),
        icon: store.archived ? ArchiveRestore : Archive,
        destructive: !store.archived,
        separated: true,
        disabled: isCash ? t('errors.CASH_LOCKED') : false,
        run: () => archive.mutate({ id: store.id, archived: !store.archived }),
      },
    ]
  }

  function groupActions(info: WalletKindInfo, count: number): UiAction[] {
    return [
      {
        id: 'add',
        label: t('stores.addOfKind', { kind: t(`stores.kind.${info.kind}`) }),
        icon: Plus,
        hidden: !canAddOfKind(info, count),
        run: () => openCreate(info.kind),
      },
      { id: 'collapse', label: t('actions.collapseAll'), icon: ChevronsDownUp, run: collapseAll },
    ]
  }

  return { subParent, subOpen, txStore, txOpen, walletActions, groupActions }
}
