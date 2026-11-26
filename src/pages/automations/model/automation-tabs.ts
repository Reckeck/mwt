/**
 * «Automations»: the user's own and the journal on the strip; schemes,
 * reminders, alerts and sources behind «More», in this order.
 */
export type AutomationTab = 'mine' | 'journal' | 'workflows' | 'reminders' | 'alerts' | 'sources'

export const TAB_ROUTE: Record<AutomationTab, string> = {
  mine: 'automations',
  journal: 'automations-journal',
  workflows: 'automations-workflows',
  reminders: 'automations-reminders',
  alerts: 'automations-alerts',
  sources: 'automations-sources',
}

export const MAIN_TABS: AutomationTab[] = ['mine', 'journal']
export const MORE_TABS: AutomationTab[] = ['workflows', 'reminders', 'alerts', 'sources']
