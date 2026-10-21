import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { entityActions, requestEntityDialog } from '@/shared/lib/entity-actions'
import type { UiAction } from '@/shared/ui/context-menu'

/** The registry as menu items for a card or a link's menu. */
export function useEntityActions() {
  const router = useRouter()
  const { t } = useI18n()
  return (target: EntityRef, title: string): UiAction[] =>
    entityActions(target.kind).map((action) => ({
      id: action.id,
      label: t(action.labelKey),
      icon: action.icon,
      run: () => {
        if (action.to) void router.push(action.to(target))
        else if (action.dialog) requestEntityDialog({ dialog: action.dialog, target, title })
      },
    }))
}
