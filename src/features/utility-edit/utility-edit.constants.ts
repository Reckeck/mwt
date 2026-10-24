import { House, Scale, Wallet } from 'lucide-vue-next'

export const UTILITY_KINDS = ['metered', 'static', 'housing'] as const
export type UtilityKindName = (typeof UTILITY_KINDS)[number]

/** The icon of each kind's card on step 1 (Wallets-3 · 14). */
export const KIND_ICONS = { metered: Scale, static: Wallet, housing: House } as const

/** How many readings a meter has (Wallets-3 · 15): one, day and night, three zones. */
export const ZONE_COUNTS = ['one', 'two', 'three'] as const
export type ZoneCount = (typeof ZONE_COUNTS)[number]

/** A choice card: an outline `Button` laid out as a card (as the new-wallet wizard's). */
export const PICK_CARD_CLASS =
  'h-auto flex-col items-start justify-start gap-1 whitespace-normal p-3 text-left font-normal'
