/** `What's new` of 21–22 October: after the newest entries, before the older archive. */
import type { WhatsNewEntry } from './whats-new.types'

export const WHATS_NEW_AUTUMN: WhatsNewEntry[] = [
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
