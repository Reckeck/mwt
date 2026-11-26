import type { IconBadgeSize, IconBadgeTone } from './icon-badge.types'

/** Circle size and glyph size per step; the glyph stroke is 1.75 on every step (v2 Icons). */
export const ICON_BADGE_SIZE_CLASS: Record<IconBadgeSize, { circle: string; glyph: string }> = {
  sm: { circle: 'size-8', glyph: 'size-4' },
  md: { circle: 'size-10', glyph: 'size-5' },
  lg: { circle: 'size-12', glyph: 'size-6' },
}

export const ICON_BADGE_TONE_CLASS: Record<IconBadgeTone, string> = {
  neutral: 'bg-muted text-muted-foreground',
  positive: 'bg-success-soft text-success',
}

export const ICON_BADGE_STROKE_WIDTH = 1.75
