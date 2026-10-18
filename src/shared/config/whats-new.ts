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
    id: 'vaultTransfer',
    date: '2026-10-19',
    points: [{ key: 'how' }, { key: 'code' }, { key: 'trace' }, { key: 'names' }],
  },
  {
    id: 'reportsRework',
    date: '2026-10-18',
    points: [
      { key: 'overview', route: 'analytics' },
      { key: 'sections', route: 'analytics-flow' },
      { key: 'insights', route: 'analytics-flow' },
      { key: 'filter', route: 'analytics-flow' },
      { key: 'phone' },
      { key: 'graph' },
      { key: 'journal', route: 'workflows-journal' },
      { key: 'contacts', route: 'transactions-contacts' },
      { key: 'nav' },
    ],
  },
  {
    id: 'lanSyncReach',
    date: '2026-10-18',
    points: [{ key: 'callback' }, { key: 'firewall' }, { key: 'background' }],
  },
  {
    id: 'lanSync',
    date: '2026-10-18',
    points: [{ key: 'lan' }, { key: 'pairing' }, { key: 'merge' }, { key: 'undo' }, { key: 'deviceSettings' }],
  },
  {
    id: 'vaultFiles',
    date: '2026-10-17',
    points: [{ key: 'androidOpen' }, { key: 'externalCopy' }, { key: 'names' }],
  },
  {
    id: 'workflows2',
    date: '2026-10-14',
    points: [
      { key: 'quickAddOnly', route: 'workflow-new' },
      { key: 'flowsInInspector', route: 'workflow-new' },
      { key: 'dataOrder', route: 'workflow-new' },
      { key: 'branches', route: 'workflows-journal' },
      { key: 'errorPorts', route: 'workflow-new' },
      { key: 'runFrom', route: 'workflow-new' },
      { key: 'queueNode', route: 'workflow-new' },
      { key: 'meterFormula', route: 'wallet' },
      { key: 'search', route: 'dashboard' },
      { key: 'utilityReminders', route: 'wallet' },
      { key: 'time24' },
      { key: 'selects' },
    ],
  },
  {
    id: 'interface',
    date: '2026-10-14',
    points: [
      { key: 'toasts' },
      { key: 'undo', route: 'stores' },
      { key: 'loading' },
      { key: 'portfolio', route: 'analytics-investments' },
      { key: 'automations', route: 'planning-automations' },
      { key: 'widgets', route: 'dashboard' },
      { key: 'numbers', route: 'wallet' },
      { key: 'menu' },
      { key: 'walletSettings', route: 'wallet-settings' },
      { key: 'tables', route: 'analytics-flow' },
      { key: 'moved', route: 'analytics-investments' },
      { key: 'palette' },
      { key: 'dates' },
      { key: 'filters', route: 'transactions' },
      { key: 'pickers' },
      { key: 'header' },
    ],
  },
  {
    id: 'round15',
    date: '2026-10-13',
    points: [
      { key: 'confirm', route: 'workflows-journal' },
      { key: 'emulation', route: 'workflow-new' },
      { key: 'hotkeys', route: 'workflow-new' },
      { key: 'nodes', route: 'workflow-new' },
      { key: 'loops', route: 'workflow-new' },
      { key: 'payday', route: 'workflows' },
      { key: 'chain', route: 'workflow-new' },
      { key: 'drafts', route: 'workflow-new' },
      { key: 'currencies', route: 'dashboard' },
      { key: 'bybitWidget', route: 'dashboard' },
      { key: 'bybitSettings', route: 'wallet-settings' },
      { key: 'pickers' },
      { key: 'adjustment', route: 'wallet' },
      { key: 'exchange', route: 'wallet' },
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
      { key: 'queue', route: 'workflows-journal' },
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
      { key: 'contacts', route: 'transactions-contacts' },
      { key: 'goals', route: 'planning-goals' },
    ],
  },
]
