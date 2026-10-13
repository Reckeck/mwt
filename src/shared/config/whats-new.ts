/**
 * «Что нового»: what changed for the user, newest first. Shown once after an
 * update (seen ids live in the app settings), and kept in the learning
 * centre. Every visible change of behaviour adds a point here — see the
 * `onboarding` skill. Texts: `whatsNew.<id>.title` and
 * `whatsNew.<id>.<point>` in both locales.
 */
export interface WhatsNewPoint {
  /** i18n key under `whatsNew.<entry id>.`. */
  key: string
  /** Route whose tour shows it; omitted when there is nothing to point at. */
  route?: string
}

export interface WhatsNewEntry {
  id: string
  /** `YYYY-MM-DD`. */
  date: string
  points: WhatsNewPoint[]
}

export const WHATS_NEW: WhatsNewEntry[] = [
  {
    id: 'round15',
    date: '2026-10-13',
    points: [
      { key: 'pickers' },
      { key: 'adjustment', route: 'wallet' },
      { key: 'settings' },
    ],
  },
  {
    id: 'rename',
    date: '2026-10-13',
    points: [{ key: 'name' }, { key: 'command' }],
  },
  {
    id: 'round14',
    date: '2026-10-13',
    points: [
      { key: 'workflows', route: 'workflows' },
      { key: 'editor', route: 'workflow-new' },
      { key: 'spacePan', route: 'workflow-new' },
      { key: 'emulation', route: 'workflow-new' },
      { key: 'lessons', route: 'workflows' },
      { key: 'queue', route: 'workflows-queue' },
      { key: 'subflows', route: 'workflow-new' },
      { key: 'tests', route: 'workflow-new' },
      { key: 'workday', route: 'workflow-new' },
    ],
  },
  {
    id: 'round13',
    date: '2026-10-12',
    points: [
      { key: 'vault' },
      { key: 'adjustment', route: 'wallet' },
      { key: 'p2p', route: 'transactions' },
      { key: 'picker', route: 'transactions' },
      { key: 'assetCurrencies', route: 'transactions' },
      { key: 'forms' },
      { key: 'notifications', route: 'planning-reminders' },
      { key: 'autoReminders', route: 'planning-reminders' },
      { key: 'reminders', route: 'planning-reminders' },
      { key: 'simulation', route: 'planning-automations' },
      { key: 'storeChange', route: 'wallet-settings' },
      { key: 'bank', route: 'stores' },
      { key: 'exchange', route: 'wallet' },
      { key: 'utilities', route: 'wallet' },
      { key: 'contacts', route: 'directory-contacts' },
      { key: 'goals', route: 'planning-goals' },
    ],
  },
]
