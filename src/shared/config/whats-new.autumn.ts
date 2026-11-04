/** `What's new` of 21–22 October (the 22nd entries first): after the newest entries, before the older archive. */
import type { WhatsNewEntry } from './whats-new.types'

export const WHATS_NEW_AUTUMN: WhatsNewEntry[] = [
  {
    id: 'entityNavigation',
    date: '2026-10-22',
    points: [
      { key: 'palette', route: 'dashboard' },
      { key: 'pinned', route: 'transactions' },
      { key: 'wallet', route: 'stores' },
      { key: 'drill', route: 'money' },
    ],
  },
  {
    id: 'workflowTemplates',
    date: '2026-10-22',
    points: [
      { key: 'gallery', route: 'automations' },
      { key: 'confirm', route: 'automations' },
      { key: 'nodes', route: 'automations-workflows' },
    ],
  },
  {
    id: 'dataSources',
    date: '2026-10-22',
    points: [
      { key: 'pick', route: 'automations-sources' },
      { key: 'uses', route: 'automations-sources' },
      { key: 'guard', route: 'automations-sources' },
      { key: 'workflows', route: 'automations' },
    ],
  },
  {
    id: 'propertyLinks',
    date: '2026-10-22',
    points: [
      { key: 'field', route: 'transactions' },
      { key: 'rent', route: 'automations' },
      { key: 'summary', route: 'stores' },
    ],
  },
  {
    id: 'propertyValuations',
    date: '2026-10-22',
    points: [
      { key: 'history', route: 'wallet-settings' },
      { key: 'rule', route: 'wallet-settings' },
      { key: 'purchase', route: 'wallet-settings' },
    ],
  },
  {
    id: 'propertyTypes',
    date: '2026-10-22',
    points: [
      { key: 'types', route: 'wallet-settings' },
      { key: 'car', route: 'wallet-settings' },
      { key: 'taxes', route: 'automations-reminders' },
    ],
  },
  {
    id: 'bonuses',
    date: '2026-10-22',
    points: [
      { key: 'tab', route: 'transactions-bonuses' },
      { key: 'wallets', route: 'transactions-bonuses' },
    ],
  },
  {
    id: 'assetClasses',
    date: '2026-10-21',
    points: [
      { key: 'advisor', route: 'money-investments' },
      { key: 'wallet', route: 'wallet-settings' },
      { key: 'networth', route: 'dashboard' },
    ],
  },
  { id: 'health', date: '2026-10-21', points: [{ key: 'light', route: 'money-health' }] },
  {
    id: 'passive',
    date: '2026-10-21',
    points: [
      { key: 'categories', route: 'settings:accounting:categories' },
      { key: 'flag', route: 'settings:accounting:categories' },
    ],
  },
  { id: 'loanKinds', date: '2026-10-21', points: [{ key: 'networth', route: 'dashboard' }] },
  {
    id: 'emergency',
    date: '2026-10-21',
    points: [
      { key: 'essential', route: 'settings:accounting:categories' },
      { key: 'fund', route: 'wallet-settings' },
    ],
  },
]
