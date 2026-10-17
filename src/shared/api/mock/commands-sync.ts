/**
 * Commands: device-to-device sync (`crate::sync` in Rust). The browser has
 * no network peers, so a scripted phone plays one: turning sync on finds it,
 * «Сопрячь» shows a code, confirming it shows a list of changes, applying it
 * adds a batch to the history.
 */

import { resolve } from './core'
import type { MockCommands } from './core'
import { mockAppSettings } from './vault-data'

const PHONE = { device_id: 'mock-phone', name: 'Pixel 8' }
const SESSION = 'mock-session'

const summary: SyncSummary = {
  total: 4,
  collections: [
    {
      collection: 'transactions',
      added: 2,
      updated: 1,
      deleted: 0,
      items: [
        { kind: 'added', label: 'Кофе' },
        { kind: 'added', label: 'Такси' },
        { kind: 'updated', label: 'Аренда' },
      ],
    },
    {
      collection: 'reminders',
      added: 0,
      updated: 0,
      deleted: 1,
      items: [{ kind: 'deleted', label: 'Позвонить в банк' }],
    },
  ],
  overwritten: [{ collection: 'stores', label: 'Наличные', field: 'name', before: 'Кошелёк', after: 'Наличные' }],
}

const state = {
  phase: null as SyncPhase | null,
  peers: [] as SyncPeerView[],
  history: [] as AppliedBatch[],
}

function session(): SyncSessionView[] {
  if (!state.phase) return []
  return [
    {
      session_id: SESSION,
      device_id: PHONE.device_id,
      name: PHONE.name,
      phase: state.phase,
      unreliable: false,
      clock_offset_ms: 0,
      progress: null,
      focused: false,
      pairing_code: state.phase === 'pairing' ? '482913' : null,
      plan: state.phase === 'awaiting_confirm' ? { reason: 'paired', summary } : null,
    },
  ]
}

function status(): SyncStatus {
  const prefs = mockAppSettings.sync
  return {
    enabled: prefs.enabled,
    role: prefs.enabled ? 'main' : 'off',
    device_id: mockAppSettings.device_id,
    device_name: mockAppSettings.device_name ?? 'Browser',
    confirm_first: prefs.confirm_first,
    prefer_main: prefs.prefer_main,
    key_storage: prefs.enabled ? 'system' : '',
    sessions: session(),
    has_backup: state.history.length > 0,
  }
}

export const syncCommands: MockCommands = {
  sync_status: () => resolve(status()),
  sync_set_settings: ({ body }) => {
    const input = body as SyncSettingsInput
    const prefs = mockAppSettings.sync
    if (input.enabled !== null && input.enabled !== undefined) prefs.enabled = input.enabled
    if (input.confirm_first !== null && input.confirm_first !== undefined) prefs.confirm_first = input.confirm_first
    if (input.prefer_main !== null && input.prefer_main !== undefined) prefs.prefer_main = input.prefer_main
    if (input.device_name !== null && input.device_name !== undefined)
      mockAppSettings.device_name = input.device_name.trim() || null
    // The scripted phone shows up once sync is on and it is not paired yet.
    state.phase = prefs.enabled ? (state.peers.length ? 'live' : 'offer') : null
    return resolve(status())
  },
  sync_pair: () => {
    state.phase = 'pairing'
    return resolve(null)
  },
  sync_confirm_pairing: ({ body }) => {
    const accept = Boolean((body as SyncDecisionInput).accept)
    if (accept) {
      state.peers = [{ ...PHONE, paired_at: new Date().toISOString(), last_synced_at: null, online: true }]
      state.phase = 'awaiting_confirm'
    } else state.phase = null
    return resolve(null)
  },
  sync_decide_plan: ({ body }) => {
    const accept = Boolean((body as SyncDecisionInput).accept)
    state.phase = accept ? 'live' : null
    if (accept) {
      const now = new Date().toISOString()
      state.history = [
        { id: `batch-${state.history.length + 1}`, peer_name: PHONE.name, applied_at: now, summary, undone_at: null },
        ...state.history,
      ]
      state.peers = state.peers.map((peer) => ({ ...peer, last_synced_at: now }))
    }
    return resolve(null)
  },
  sync_peers: () => resolve(state.peers.map((peer) => ({ ...peer, online: state.phase === 'live' }))),
  sync_forget_peer: () => {
    state.peers = []
    state.phase = null
    return resolve(null)
  },
  sync_history: () => resolve(state.history),
  sync_undo: ({ args }) => {
    const id = String(args?.batchId ?? '')
    state.history = state.history.map((batch) =>
      batch.id === id ? { ...batch, undone_at: new Date().toISOString() } : batch,
    )
    return resolve<UndoReport>({ restored: 3, skipped: ['Аренда'] })
  },
  sync_restore_backup: () => resolve(null),
}
