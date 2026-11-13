/** `What's new` of 1–5 November (sync, contacts, updates): the newest entries, before the October ones. */
import type { WhatsNewEntry } from './whats-new.types'

export const WHATS_NEW_NOVEMBER: WhatsNewEntry[] = [
  {
    id: 'exchangeVia',
    date: '2026-11-14',
    points: [
      { key: 'via', route: 'transactions' },
      { key: 'split', route: 'transactions' },
    ],
  },
  {
    id: 'phoneAutomations',
    date: '2026-11-13',
    points: [
      { key: 'bar', route: 'dashboard' },
      { key: 'actions', route: 'automations' },
      { key: 'repeat', route: 'automations-reminders' },
    ],
  },
  {
    id: 'syncProtocol4',
    date: '2026-11-10',
    points: [
      { key: 'update', route: 'settings:sync:sync-devices' },
      { key: 'private', route: 'settings:sync:sync-devices' },
    ],
  },
  {
    id: 'vaultProtection',
    date: '2026-11-08',
    points: [
      { key: 'levels', route: 'settings:vault' },
      { key: 'autolock', route: 'settings:vault' },
      { key: 'deviceKeys', route: 'settings:vault' },
      { key: 'quickUnlock', route: 'settings:vault' },
      { key: 'syncLevel', route: 'settings:sync:sync-devices' },
    ],
  },
  {
    id: 'syncStageE',
    date: '2026-11-05',
    points: [{ key: 'platform', route: 'settings:sync:sync-devices' }],
  },
  {
    id: 'syncStageP',
    date: '2026-11-05',
    points: [
      { key: 'settings', route: 'settings:sync:sync-devices' },
      { key: 'inflation', route: 'settings:accounting:inflation' },
      { key: 'notices', route: 'settings:app:notifications' },
      { key: 'reminderText', route: 'settings:app:notifications' },
      { key: 'layout', route: 'dashboard' },
      { key: 'cap', route: 'automations-workflows' },
    ],
  },
  {
    id: 'syncStageS',
    date: '2026-11-04',
    points: [
      { key: 'restore', route: 'settings:vault' },
      { key: 'editor', route: 'settings:sync:sync-devices' },
      { key: 'external', route: 'settings:sync:sync-devices' },
    ],
  },
  {
    id: 'syncStageM',
    date: '2026-11-04',
    points: [
      { key: 'noDoubles', route: 'settings:sync:sync-devices' },
      { key: 'keep', route: 'transactions' },
      { key: 'protocol', route: 'settings:sync:sync-devices' },
    ],
  },
  {
    id: 'appUpdates',
    date: '2026-11-03',
    points: [
      { key: 'auto', route: 'settings:app:updates' },
      { key: 'manual', route: 'settings:app:updates' },
    ],
  },
  {
    id: 'contactsSection',
    date: '2026-11-03',
    points: [
      { key: 'section', route: 'contacts' },
      { key: 'table', route: 'contacts' },
      { key: 'sheet', route: 'contacts' },
      { key: 'requisite', route: 'contacts' },
    ],
  },
  {
    id: 'phoneRequisites',
    date: '2026-11-03',
    points: [
      { key: 'recognize', route: 'contacts' },
      { key: 'kind', route: 'contacts' },
    ],
  },
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
      { key: 'runOnce', route: 'settings:sync:sync-devices' },
      { key: 'deletionsStay', route: 'settings:sync:sync-devices' },
      { key: 'rolesSteady', route: 'settings:sync:sync-devices' },
      { key: 'editBanner', route: 'settings:sync:sync-devices' },
      { key: 'pairingWait', route: 'settings:sync:sync-devices' },
    ],
  },
]
