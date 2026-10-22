import type { Component } from 'vue'
import { ChartArea, LayoutDashboard, ScrollText, WalletCards, Workflow } from 'lucide-vue-next'

export interface NavItem {
  /** Route the item opens. */
  name: string
  icon: Component
  /** i18n key of the label. */
  key: string
  /** The item stays active on every path under this one (a section with tabs). */
  prefix?: string
}

/** The five sections (Shell · 7): the sidebar, the phone's bar and «More», the palette. */
export const NAV_ITEMS: NavItem[] = [
  { name: 'dashboard', icon: LayoutDashboard, key: 'nav.dashboard' },
  { name: 'transactions', icon: ScrollText, key: 'nav.transactions', prefix: '/transactions' },
  { name: 'stores', icon: WalletCards, key: 'nav.stores', prefix: '/stores' },
  { name: 'money', icon: ChartArea, key: 'nav.money', prefix: '/money' },
  { name: 'automations', icon: Workflow, key: 'nav.automations', prefix: '/automations' },
]

export const navItem = (name: string) => NAV_ITEMS.find((item) => item.name === name) as NavItem
