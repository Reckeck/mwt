import { ListTree, PanelRight } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import type { UiAction } from '@/shared/ui/context-menu'

import { operationsQuery } from '@/entities/crypto-position'

/** The «⋯» menu of a row: the same two things on a phone and on a desktop, none of them hover-only. */
export function usePositionActions(open: (position: CryptoPositionView) => void) {
  const { t } = useI18n()
  const router = useRouter()
  return (position: CryptoPositionView): UiAction[] => [
    { id: 'open', label: t('cryptoTrack.positions.actions.open'), icon: PanelRight, run: () => open(position) },
    {
      id: 'operations',
      label: t('cryptoTrack.positions.actions.operations'),
      hint: t('cryptoTrack.positions.actions.operationsHint'),
      icon: ListTree,
      opens: true,
      run: () => void router.push({ name: 'crypto-operations', query: operationsQuery(position) }),
    },
  ]
}
