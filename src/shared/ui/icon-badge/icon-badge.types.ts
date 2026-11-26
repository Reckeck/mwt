import type { Component } from 'vue'

export type IconBadgeSize = 'sm' | 'md' | 'lg'
export type IconBadgeTone = 'neutral' | 'positive'

export interface IconBadgeProps {
  /** The glyph: a lucide component (see `store-icons.ts` for the store ones). */
  icon: Component
  size?: IconBadgeSize
  /** `positive` (soft green) marks income; `neutral` is the sunken surface. */
  tone?: IconBadgeTone
}
