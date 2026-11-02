/**
 * `What's new`: what changed for the user, newest first. Shown once after an
 * update (seen ids live in the app settings), and kept in the learning
 * centre. Every visible change of behaviour adds a point here — see the
 * `onboarding` skill. Texts: `whatsNew.<id>.title` and
 * `whatsNew.<id>.<point>` in both locales.
 */
import type { WhatsNewEntry } from './whats-new.types'
// The extension lets tests/checks/onboarding.check.ts load this file.
import { WHATS_NEW_ARCHIVE } from './whats-new.archive.ts'
import { WHATS_NEW_AUTUMN } from './whats-new.autumn.ts'

export type { WhatsNewEntry, WhatsNewPoint } from './whats-new.types'

export const WHATS_NEW: WhatsNewEntry[] = [
  {
    id: 'syncTransfer',
    date: '2026-11-03',
    points: [
      { key: 'unlock', route: 'unlock' },
      { key: 'transfer', route: 'settings:sync:sync-transfer' },
      { key: 'confirm', route: 'settings:sync:sync-devices' },
      { key: 'forgotten', route: 'settings:sync:sync-devices' },
    ],
  },
  {
    id: 'syncRepair',
    date: '2026-11-01',
    points: [
      { key: 'forgotten', route: 'settings:sync:sync-devices' },
      { key: 'paused', route: 'settings:sync:sync-devices' },
      { key: 'cutOff', route: 'settings:sync:sync-devices' },
      { key: 'externalCopy', route: 'settings:sync:sync-devices' },
      { key: 'versions', route: 'settings:sync:sync-devices' },
      { key: 'laterRecords', route: 'settings:sync:sync-devices' },
      { key: 'duplicates', route: 'settings:sync:sync-devices' },
    ],
  },
  {
    id: 'togetherEvents',
    date: '2026-10-29',
    points: [
      { key: 'create', route: 'money-together' },
      { key: 'items', route: 'money-together-event' },
      { key: 'split', route: 'money-together-event' },
      { key: 'guests', route: 'money-together-event' },
      { key: 'payment', route: 'transactions' },
      { key: 'sidebar', route: 'dashboard' },
    ],
  },
  {
    id: 'togetherSettle',
    date: '2026-10-29',
    points: [
      { key: 'settle', route: 'money-together-event' },
      { key: 'payShare', route: 'money-together-event' },
      { key: 'share', route: 'money-together-event' },
      { key: 'contact', route: 'transactions-contacts' },
      { key: 'dashboard', route: 'dashboard' },
    ],
  },
  {
    id: 'quickContacts',
    date: '2026-10-29',
    points: [{ key: 'create' }, { key: 'fillIn', route: 'transactions-contacts' }],
  },
  {
    id: 'amountCalculator',
    date: '2026-10-29',
    points: [{ key: 'field', route: 'transactions' }, { key: 'note' }],
  },
  {
    id: 'appIcon',
    date: '2026-10-29',
    points: [{ key: 'icon' }],
  },
  {
    id: 'twoPayouts',
    date: '2026-10-28',
    points: [
      { key: 'payouts', route: 'automations' },
      { key: 'forMonth', route: 'transactions' },
    ],
  },
  {
    id: 'homeFixes',
    date: '2026-10-28',
    points: [
      { key: 'changes', route: 'dashboard' },
      { key: 'leftToday', route: 'dashboard' },
      { key: 'categories', route: 'settings:accounting:categories' },
    ],
  },
  {
    id: 'subAccountTiles',
    date: '2026-10-28',
    points: [{ key: 'tiles' }, { key: 'table', route: 'stores' }],
  },
  {
    id: 'newSettings',
    date: '2026-10-25',
    points: [
      { key: 'settings', route: 'settings:app' },
      // The sign-in screen exists only before a vault is open: no link.
      { key: 'login' },
      { key: 'plain', route: 'settings:vault' },
      { key: 'sandbox' },
      { key: 'history', route: 'settings:sync:sync-history' },
      { key: 'pair', route: 'settings:sync:sync-devices' },
    ],
  },
  {
    id: 'newAutomations',
    date: '2026-10-25',
    points: [
      { key: 'mine', route: 'automations' },
      { key: 'card', route: 'automations' },
      { key: 'journal', route: 'automations-journal' },
      { key: 'wizard', route: 'automations' },
      { key: 'scheme', route: 'workflow-new' },
      { key: 'emulate', route: 'workflow-new' },
      { key: 'import', route: 'automations' },
    ],
  },
  {
    id: 'newCurrency',
    date: '2026-10-24',
    points: [
      { key: 'roles' },
      { key: 'sum', route: 'transactions' },
      { key: 'exchange', route: 'transactions' },
      { key: 'incomplete', route: 'money' },
      { key: 'stale', route: 'dashboard' },
      { key: 'palette' },
    ],
  },
  {
    id: 'newWallets',
    date: '2026-10-24',
    points: [
      { key: 'table', route: 'stores' },
      { key: 'wizard', route: 'stores' },
      { key: 'pages', route: 'stores' },
      { key: 'utilities', route: 'stores' },
      { key: 'reconcile', route: 'stores' },
    ],
  },
  {
    id: 'newFiles',
    date: '2026-10-24',
    points: [
      { key: 'attach', route: 'transactions' },
      { key: 'filter', route: 'transactions' },
      { key: 'goal', route: 'money-plan' },
      { key: 'delete', route: 'money-plan' },
    ],
  },
  {
    id: 'newOps',
    date: '2026-10-24',
    points: [
      { key: 'select', route: 'transactions' },
      { key: 'calendar', route: 'transactions' },
      { key: 'contacts', route: 'transactions' },
      { key: 'form' },
      { key: 'deal', route: 'stores' },
    ],
  },
  {
    id: 'newPaths',
    date: '2026-10-23',
    points: [
      { key: 'glance', route: 'dashboard' },
      { key: 'back', route: 'money' },
      { key: 'palette' },
      { key: 'menu', route: 'transactions' },
      { key: 'auto', route: 'transactions' },
    ],
  },
  {
    id: 'newMoney',
    date: '2026-10-23',
    points: [
      { key: 'where', route: 'money-where' },
      { key: 'plan', route: 'money-plan' },
      { key: 'wealth', route: 'money-wealth' },
      { key: 'calculator', route: 'money-calculator' },
    ],
  },
  {
    id: 'newHome',
    date: '2026-10-23',
    points: [
      { key: 'figures', route: 'dashboard' },
      { key: 'tasks', route: 'dashboard' },
      { key: 'changes', route: 'dashboard' },
      { key: 'widgets', route: 'dashboard' },
    ],
  },
  {
    id: 'newShell',
    date: '2026-10-23',
    points: [
      { key: 'sections', route: 'dashboard' },
      { key: 'fold', route: 'dashboard' },
      { key: 'spending', route: 'money' },
      { key: 'month', route: 'money-month' },
      { key: 'automations', route: 'automations' },
    ],
  },
  {
    id: 'vaultSetup',
    date: '2026-10-22',
    points: [{ key: 'currency' }, { key: 'direction', route: 'settings:accounting:direction' }],
  },
  {
    id: 'dashboardNew',
    date: '2026-10-22',
    points: [
      { key: 'today', route: 'dashboard' },
      { key: 'widgets', route: 'dashboard' },
      { key: 'scenario', route: 'dashboard' },
      { key: 'reset', route: 'dashboard' },
    ],
  },
  {
    id: 'dashboardWidgets',
    date: '2026-10-22',
    points: [
      { key: 'month', route: 'dashboard' },
      { key: 'limits', route: 'dashboard' },
      { key: 'upcoming', route: 'dashboard' },
      { key: 'markets', route: 'dashboard' },
    ],
  },
  {
    id: 'dashboardTiles',
    date: '2026-10-22',
    points: [
      { key: 'period', route: 'dashboard' },
      { key: 'merged', route: 'dashboard' },
      { key: 'size', route: 'dashboard' },
    ],
  },
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
  ...WHATS_NEW_AUTUMN,
  ...WHATS_NEW_ARCHIVE,
]
