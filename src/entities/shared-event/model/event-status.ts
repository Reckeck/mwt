import type { BadgeVariant } from '@/shared/ui/badge'

/** The status badge of an event: closed reads as done, a debt that hangs is a warning. */
export const EVENT_STATUS_VARIANT: Record<SharedEventStatus, BadgeVariant> = {
  planning: 'secondary',
  in_progress: 'secondary',
  settling: 'warning',
  closed: 'success',
}

/** i18n key of the status word: `sharedEvents.status.<status>`. */
export const eventStatusKey = (status: SharedEventStatus) => `sharedEvents.status.${status}`
