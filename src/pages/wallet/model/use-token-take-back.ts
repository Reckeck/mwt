import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Undo2 } from 'lucide-vue-next'

import { useTokenReview } from '@/features/crypto-token-review'
import { useCryptoTokens } from '@/entities/crypto-token'
import type { UiAction } from '@/shared/ui/context-menu'

/**
 * The lasting way to un-recognise a token (spec E9): a row action on the assets table of an address
 * for a token the user confirmed, asking first because it leaves the address's value, its profit and
 * the reports. The toast's `Undo` is only the first seconds; this is the week-later case.
 */
export function useTokenTakeBack() {
  const { t } = useI18n()
  const tokens = useCryptoTokens()
  const { clear } = useTokenReview()
  const asking = ref<TokenView | null>(null)
  const open = computed({
    get: () => asking.value !== null,
    set: (value: boolean) => {
      if (!value) asking.value = null
    },
  })

  /** The token a table row is, when the user recognised it: only that kind has a verdict to take back. */
  function confirmedToken(position: StorePosition): TokenView | undefined {
    const contract = position.asset.contract?.toLowerCase()
    if (!contract) return undefined
    return (tokens.data.value ?? []).find(
      (token) =>
        token.status === 'confirmed' &&
        token.contract.toLowerCase() === contract &&
        token.chain === position.asset.chain,
    )
  }
  /** The row's menu; empty for a coin, a built-in token or one that is still in quarantine. */
  function rowActions(position: StorePosition): UiAction[] {
    const token = confirmedToken(position)
    if (!token) return []
    return [
      {
        id: 'take-back',
        label: t('cryptoTrack.assets.takeBack'),
        icon: Undo2,
        destructive: true,
        run: () => (asking.value = token),
      },
    ]
  }
  function confirm() {
    const token = asking.value
    if (!token) return
    clear.mutate({ chain: token.chain, contract: token.contract }, { onSuccess: () => (asking.value = null) })
  }
  return { rowActions, asking, open, confirm, pending: computed(() => clear.isPending.value) }
}
