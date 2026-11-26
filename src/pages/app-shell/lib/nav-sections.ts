import type { Component } from 'vue'
import { ChartPie, House, List, TrendingUp, Users } from 'lucide-vue-next'

export interface NavSection {
  id: 'home' | 'operations' | 'budget' | 'people' | 'capital'
  icon: Component
  /** i18n key of the label. */
  key: string
  /** Route the section opens. */
  root: string
  /** Every route name that belongs to the section (the active state matches these). */
  routes: readonly string[]
}

/** The five sections (spec D4) and the route names under each; names and paths are unchanged. */
export const NAV_SECTIONS: readonly NavSection[] = [
  { id: 'home', icon: House, key: 'nav.section.home', root: 'dashboard', routes: ['dashboard'] },
  {
    id: 'operations',
    icon: List,
    key: 'nav.section.operations',
    root: 'transactions',
    routes: ['transactions', 'transactions-calendar', 'transactions-bonuses', 'crypto-operations'],
  },
  {
    id: 'budget',
    icon: ChartPie,
    key: 'nav.section.budget',
    root: 'money',
    routes: ['money', 'money-month', 'money-plan', 'money-where', 'money-calculator', 'money-health'],
  },
  {
    id: 'people',
    icon: Users,
    key: 'nav.section.people',
    root: 'contacts',
    routes: ['contacts', 'money-together', 'money-together-event'],
  },
  {
    id: 'capital',
    icon: TrendingUp,
    key: 'nav.section.capital',
    root: 'stores',
    routes: ['stores', 'wallet', 'wallet-settings', 'money-wealth', 'money-investments', 'crypto-positions'],
  },
]

/** Routes reached from the profile menu, not from a section (settings, automations, the vault screen). */
export const PROFILE_MENU_ROUTES: readonly string[] = [
  'automations',
  'automations-journal',
  'automations-workflows',
  'automations-reminders',
  'automations-alerts',
  'automations-sources',
  'workflow-new',
  'workflow-edit',
  'settings',
  'unlock',
]

/** The section a route name belongs to; `undefined` for the profile menu and unknown names. */
export function sectionOfRoute(name: string): NavSection | undefined {
  return NAV_SECTIONS.find((section) => section.routes.includes(name))
}
