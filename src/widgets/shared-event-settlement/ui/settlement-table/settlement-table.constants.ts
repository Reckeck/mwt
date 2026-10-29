import type { PersonStatus } from '@/entities/shared-event'
import type { BadgeVariant } from '@/shared/ui/badge'

/** The badge of a person's row: returned and closed are settled, a part left is a warning. */
export const STATUS_BADGE: Record<Exclude<PersonStatus, 'organizer' | 'even'>, { variant: BadgeVariant }> = {
  waiting: { variant: 'secondary' },
  partial: { variant: 'warning' },
  returned: { variant: 'success' },
  closed: { variant: 'success' },
}
