import { type Ref, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowLeftRight, Eye, EyeOff, ExternalLink, FileText, Tag } from 'lucide-vue-next'

import { openOperationInBrowser } from '@/entities/crypto-tx'
import { notify } from '@/shared/lib/notify'
import type { UiAction } from '@/shared/ui/context-menu'

import { useHideOperation } from './use-crypto-override'

/**
 * The menu of an operation (`⋯`), and the layers it opens. There is nothing in it that creates or deletes an
 * operation: the module only reads. `Hide from totals` is a correction with an Undo, the rest opens a dialog
 * or the explorer (on this click, not before).
 *
 * The module's own entries (an opening balance, a correction towards what the chain reports) are not
 * operations of a chain: there is nothing to exchange, nothing to classify and no page to open, and hiding
 * one would make the total disagree with the network on purpose. They keep the description alone.
 */
export function useCryptoActions() {
  const { t } = useI18n()
  const hide = useHideOperation()
  const exchangeFor: Ref<CryptoOpRow | null> = ref(null)
  const exchangeOpen = ref(false)
  const meaningFor: Ref<CryptoOpRow | null> = ref(null)
  const meaningOpen = ref(false)
  const meaningFocus: Ref<'category' | 'description'> = ref('category')

  function askExchange(row: CryptoOpRow) {
    exchangeFor.value = row
    exchangeOpen.value = true
  }
  function askMeaning(row: CryptoOpRow, focus: 'category' | 'description' = 'category') {
    meaningFor.value = row
    meaningFocus.value = focus
    meaningOpen.value = true
  }
  async function openInBrowser(row: CryptoOpRow) {
    try {
      await openOperationInBrowser(row.id)
    } catch (error) {
      notify.error(error, 'cryptoTrack.operations.openFailed')
    }
  }

  function actions(row: CryptoOpRow): UiAction[] {
    return [
      {
        id: 'exchange',
        label: t('cryptoTrack.operations.menu.exchange'),
        icon: ArrowLeftRight,
        hidden: Boolean(row.flow_id) || Boolean(row.own_entry),
        run: () => askExchange(row),
      },
      {
        id: 'category',
        label: t('cryptoTrack.operations.menu.category'),
        icon: Tag,
        hidden: Boolean(row.own_entry),
        run: () => askMeaning(row, 'category'),
      },
      {
        id: 'description',
        label: t('cryptoTrack.operations.menu.description'),
        icon: FileText,
        run: () => askMeaning(row, 'description'),
      },
      {
        id: 'hide',
        label: t(row.hidden ? 'cryptoTrack.operations.menu.show' : 'cryptoTrack.operations.menu.hide'),
        icon: row.hidden ? Eye : EyeOff,
        hidden: Boolean(row.own_entry),
        run: () => hide.mutate({ id: row.id, hidden: !row.hidden }),
      },
      {
        id: 'explorer',
        label: t('cryptoTrack.operations.menu.explorer'),
        icon: ExternalLink,
        separated: true,
        hidden: !row.linkable,
        run: () => void openInBrowser(row),
      },
    ]
  }

  return {
    actions,
    askExchange,
    askMeaning,
    openInBrowser,
    exchangeFor,
    exchangeOpen,
    meaningFor,
    meaningOpen,
    meaningFocus,
  }
}

export type CryptoActions = ReturnType<typeof useCryptoActions>
