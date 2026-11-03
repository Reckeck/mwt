import type { Component } from 'vue'
import { ChartPie, House, List, Users, Wallet, Zap } from 'lucide-vue-next'

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
  { name: 'dashboard', icon: House, key: 'nav.dashboard' },
  { name: 'transactions', icon: List, key: 'nav.transactions', prefix: '/transactions' },
  { name: 'stores', icon: Wallet, key: 'nav.stores', prefix: '/stores' },
  { name: 'money', icon: ChartPie, key: 'nav.money', prefix: '/money' },
  { name: 'automations', icon: Zap, key: 'nav.automations', prefix: '/automations' },
]

/** Contacts sit in the sidebar's bottom group and the phone's «More», not among the five sections. */
export const CONTACTS_ITEM: NavItem = { name: 'contacts', icon: Users, key: 'nav.contacts', prefix: '/contacts' }

export const navItem = (name: string) => NAV_ITEMS.find((item) => item.name === name) as NavItem
