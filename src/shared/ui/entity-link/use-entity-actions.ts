import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { Pin, PinOff } from 'lucide-vue-next'

import { usePins } from '@/shared/api/pins'
import { entityActions, requestEntityDialog } from '@/shared/lib/entity-actions'
import type { UiAction } from '@/shared/ui/context-menu'

/** The registry as menu items for a card or a link's menu. */
export function useEntityActions() {
  const router = useRouter()
  const { t } = useI18n()
  const pins = usePins()
  return (target: EntityRef, title: string): UiAction[] => [
    ...entityActions(target.kind).map((action) => ({
      id: action.id,
      label: t(action.labelKey),
      icon: action.icon,
      hidden: action.hidden?.(target) ?? false,
      run: () => {
        if (action.to) void router.push(action.to(target))
        else if (action.dialog) requestEntityDialog({ dialog: action.dialog, target, title })
      },
    })),
    pins.isPinned(target)
      ? { id: 'unpin', label: t('pins.unpin'), icon: PinOff, separated: true, run: () => pins.toggle(target) }
      : { id: 'pin', label: t('pins.pin'), icon: Pin, separated: true, run: () => pins.toggle(target) },
  ]
}
