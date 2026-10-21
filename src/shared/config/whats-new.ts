/**
 * «Что нового»: what changed for the user, newest first. Shown once after an
 * update (seen ids live in the app settings), and kept in the learning
 * centre. Every visible change of behaviour adds a point here — see the
 * `onboarding` skill. Texts: `whatsNew.<id>.title` and
 * `whatsNew.<id>.<point>` in both locales.
 */
import type { WhatsNewEntry } from './whats-new.types'
// The extension lets scripts/check-onboarding.mjs load this file with Node's type stripping.
import { WHATS_NEW_ARCHIVE } from './whats-new.archive.ts'

export type { WhatsNewEntry, WhatsNewPoint } from './whats-new.types'

export const WHATS_NEW: WhatsNewEntry[] = [
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
      { key: 'advisor', route: 'analytics-investments' },
      { key: 'wallet', route: 'wallet-settings' },
      { key: 'networth', route: 'dashboard' },
    ],
  },
  { id: 'health', date: '2026-10-21', points: [{ key: 'light', route: 'analytics-health' }] },
  {
    id: 'passive',
    date: '2026-10-21',
    points: [
      { key: 'categories', route: 'settings:categories' },
      { key: 'flag', route: 'settings:categories' },
    ],
  },
  { id: 'loanKinds', date: '2026-10-21', points: [{ key: 'networth', route: 'dashboard' }] },
  {
    id: 'emergency',
    date: '2026-10-21',
    points: [
      { key: 'essential', route: 'settings:categories' },
      { key: 'fund', route: 'wallet-settings' },
    ],
  },
  {
    id: 'goalReturn',
    date: '2026-10-21',
    points: [
      { key: 'return', route: 'planning-goals' },
      { key: 'manual', route: 'settings:money' },
    ],
  },
  {
    id: 'inflationAuto',
    date: '2026-10-21',
    points: [
      { key: 'auto', route: 'settings:money' },
      { key: 'notices', route: 'settings:notifications' },
      { key: 'keyRate' },
    ],
  },
  {
    id: 'calculator',
    date: '2026-10-21',
    points: [
      { key: 'calc', route: 'planning-calculator' },
      { key: 'reverse', route: 'planning-calculator' },
      { key: 'prefill', route: 'planning-calculator' },
      { key: 'scenarios', route: 'planning-calculator' },
      { key: 'goal', route: 'planning-goals' },
    ],
  },
  {
    id: 'links',
    date: '2026-10-21',
    points: [{ key: 'card', route: 'transactions' }, { key: 'chips' }, { key: 'notices' }, { key: 'address' }],
  },
  {
    id: 'salary',
    date: '2026-10-20',
    points: [
      { key: 'varies', route: 'planning-automations' },
      { key: 'workday', route: 'planning-automations' },
      { key: 'confirm', route: 'planning-automations' },
      { key: 'workflows', route: 'workflows' },
    ],
  },
  ...WHATS_NEW_ARCHIVE,
]
