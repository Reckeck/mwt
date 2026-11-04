/** Demo devices of the sync mock that are not the scripted phone. */

/** A second demo device that forgot this one: the list shows its badge and "Pair again". */
export const LAPTOP: SyncPeerView = {
  device_id: 'mock-laptop',
  name: 'Ноутбук',
  paired_at: '2026-10-20T20:00:00Z',
  last_synced_at: '2026-10-21T18:00:00Z',
  online: false,
  paused: false,
  cut_by: 'forgotten',
  forgotten: false,
}

/** A device the user forgot here: the list shows it under "Forgotten devices" with "Pair again". */
export const TABLET: SyncPeerView = {
  device_id: 'mock-tablet',
  name: 'Планшет',
  paired_at: '2026-10-18T12:00:00Z',
  last_synced_at: '2026-10-19T09:30:00Z',
  online: false,
  paused: false,
  cut_by: null,
  forgotten: true,
}

/** What the vault would show once the batch below is applied. */
export const after: AfterApply = {
  currency: 'rub',
  net_worth_before: '1250000',
  net_worth_after: '1247500',
  stores: [{ id: 'store-bank', name: 'Тинькофф', before: '82300', after: '79800', currency: 'rub' }],
}

export const summary: SyncSummary = {
  total: 4,
  collections: [
    {
      collection: 'transactions',
      added: 2,
      updated: 1,
      deleted: 0,
      items: [
        { kind: 'added', label: 'Продукты, 2026-11-03, -2500 RUB', key: 'tx-1', fields: [] },
        { kind: 'added', label: 'Такси, 2026-11-03, -480 RUB', key: 'tx-2', fields: [] },
        {
          kind: 'updated',
          label: 'Зарплата, 2026-11-01, +100000 RUB',
          key: 'tx-3',
          fields: [
            { field: 'amount', before: '100000', after: '105000', money_currency: 'rub' },
            { field: 'meta.comment', before: 'Зарплата', after: 'Зарплата с премией', money_currency: null },
          ],
        },
      ],
    },
    {
      collection: 'reminders',
      added: 0,
      updated: 0,
      deleted: 1,
      items: [{ kind: 'deleted', label: 'Позвонить в банк', key: 'rem-1', fields: [] }],
    },
  ],
  overwritten: [{ collection: 'stores', label: 'Наличные', field: 'name', before: 'Кошелёк', after: 'Наличные' }],
  unstamped_conflicts: 0,
}

/** What the backend already journaled before this session: applied, declined, sent. */
export const seedJournal = (): SyncJournalEntry[] => [
  {
    id: 'journal-4',
    at: '2026-10-23T08:00:00Z',
    peer_name: 'Внешняя копия',
    counts: [{ collection: 'transactions', count: 3 }],
    label: null,
    outcome: 'applied',
    batch_id: null,
  },
  {
    id: 'journal-3',
    at: '2026-10-22T09:40:00Z',
    peer_name: 'Pixel 8',
    counts: [
      { collection: 'transactions', count: 2 },
      { collection: 'budgets', count: 1 },
    ],
    label: null,
    outcome: 'applied',
    batch_id: null,
  },
  {
    id: 'journal-2',
    at: '2026-10-21T18:00:00Z',
    peer_name: 'Ноутбук',
    counts: [],
    label: null,
    outcome: 'sent',
    batch_id: null,
  },
  {
    id: 'journal-1',
    at: '2026-10-20T20:15:00Z',
    peer_name: 'Pixel 8',
    counts: [{ collection: 'transactions', count: 1 }],
    label: 'Маме до зарплаты',
    outcome: 'declined',
    batch_id: null,
  },
]
