import type { CardSize } from './card.types'

/**
 * Padding per card size. The header keeps a small bottom gap and the next
 * section drops its top padding, so a titled card never doubles the spacing.
 */
export const CARD_PADDING: Record<CardSize, { section: string; header: string }> = {
  sm: { section: 'p-3', header: 'pb-1.5' },
  md: { section: 'p-3 sm:p-4', header: 'pb-2 sm:pb-2' },
}
