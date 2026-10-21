import type { Component } from 'vue'
import {
  ArrowLeftRight,
  Bell,
  Calculator,
  CalendarClock,
  Coins,
  Gauge,
  Tag,
  Target,
  User,
  Wallet,
  Workflow,
  Zap,
} from 'lucide-vue-next'

/** One icon per kind of entity, the same in links, cards and the palette. */
export const ENTITY_ICONS: Record<EntityKind, Component> = {
  store: Wallet,
  tx: ArrowLeftRight,
  category: Tag,
  contact: User,
  goal: Target,
  budget: Gauge,
  automation: Zap,
  workflow: Workflow,
  reminder: CalendarClock,
  alert: Bell,
  asset: Coins,
  scenario: Calculator,
}

/** The link itself: inline, truncating, underlined on hover; the caller may add a colour. */
export const ENTITY_LINK_CLASS =
  'inline-flex max-w-full min-w-0 items-center gap-1 rounded-sm align-bottom text-foreground underline-offset-2 ' +
  'hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
