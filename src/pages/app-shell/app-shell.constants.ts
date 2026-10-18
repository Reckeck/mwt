import type { Component } from 'vue'
import { CalendarCheck, ChartArea, LayoutDashboard, ScrollText, WalletCards, Workflow } from 'lucide-vue-next'

export interface NavItem {
  /** Route the item opens. */
  name: string
  icon: Component
  /** i18n key of the label. */
  key: string
  /** The item stays active on every path under this one (a section with tabs). */
  prefix?: string
}

export interface NavGroup {
  /** i18n key of the group's label; the first group has none. */
  key: string | null
  items: NavItem[]
}

/** The app's sections, grouped by what one goes there for. */
export const NAV_GROUPS: NavGroup[] = [
  { key: null, items: [{ name: 'dashboard', icon: LayoutDashboard, key: 'nav.dashboard' }] },
  {
    key: 'nav.group.money',
    items: [
      { name: 'stores', icon: WalletCards, key: 'nav.stores', prefix: '/stores' },
      { name: 'transactions', icon: ScrollText, key: 'nav.transactions', prefix: '/transactions' },
      { name: 'analytics', icon: ChartArea, key: 'nav.analytics', prefix: '/analytics' },
    ],
  },
  {
    key: 'nav.group.plans',
    items: [
      { name: 'planning-automations', icon: CalendarCheck, key: 'nav.planning', prefix: '/planning' },
      { name: 'workflows', icon: Workflow, key: 'nav.workflows', prefix: '/workflows' },
    ],
  },
]
