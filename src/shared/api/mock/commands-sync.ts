/**
 * Commands: device-to-device sync (`crate::sync` in Rust). The browser has
 * no network peers, so a scripted phone plays one: turning sync on finds it,
 * "Pair" shows a code, confirming it shows a list of changes, applying it
 * adds a batch to the history.
 */

import { resolve } from './core'
import type { MockCommands } from './core'
import { emitMock } from '../mock-bus'
import { reminders } from './planning/state'
import { LAPTOP, TABLET, after, seedJournal, summary } from './sync-peers'
import { mockAppSettings } from './vault-data'

const PHONE = { device_id: 'mock-phone', name: 'Pixel 8' }
const SESSION = 'mock-session'

/** The real handshake gives the code five minutes. */
const PAIRING_MS = 5 * 60_000

const state = {
  laptop: LAPTOP as SyncPeerView | null,
  tablet: TABLET as SyncPeerView | null,
  pairingStart: 0,
  phase: null as SyncPhase | null,
  peers: [] as SyncPeerView[],
  history: [] as AppliedBatch[],
  journal: seedJournal(),
  since: Date.now(),
  firewallOpen: false,
}

/** One line of the journal for `summary`, as the backend writes it. */
function journalLine(outcome: SyncJournalOutcome, batchId: string | null): SyncJournalEntry {
  const counts = summary.collections.map((changes) => ({
    collection: changes.collection,
    count: changes.added + changes.updated + changes.deleted,
  }))
  return {
    id: `journal-${state.journal.length + 4}`,
    at: new Date().toISOString(),
    peer_name: PHONE.name,
    counts,
    label: null,
    outcome,
    batch_id: batchId,
  }
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
      pairing_expires_ms: state.phase === 'pairing' ? state.pairingStart + PAIRING_MS : null,
      plan: state.phase === 'awaiting_confirm' ? { reason: 'paired', summary, after } : null,
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
    background_supported: false,
    background_lock_min: prefs.background_lock_min ?? 30,
    network: {
      port: prefs.enabled ? 53318 : null,
      address: prefs.enabled ? '192.168.1.174' : null,
      seen: state.phase ? 1 : 0,
      since_ms: prefs.enabled ? state.since : null,
      // Shown until "Open port", as if the phone could not get in.
      inbound_blocked: prefs.enabled && !state.firewallOpen,
      firewall_fix: 'auto',
      firewall_commands: [
        "sudo ufw allow from 192.168.1.0/24 to any port 53318 proto tcp comment 'Multi Wallet Tracker sync'",
      ],
      last_error: null,
    },
  }
}

/** What the backend does after every change: pushes the status (the browser has no events of its own). */
const push = () => setTimeout(() => emitMock('sync-status', status()), 0)

export const syncCommands: MockCommands = {
  sync_status: () => resolve(status()),
  sync_set_settings: ({ body }) => {
    const input = body as SyncSettingsInput
    const prefs = mockAppSettings.sync
    if (input.enabled !== null && input.enabled !== undefined) prefs.enabled = input.enabled
    if (input.confirm_first !== null && input.confirm_first !== undefined) prefs.confirm_first = input.confirm_first
    if (input.prefer_main !== null && input.prefer_main !== undefined) prefs.prefer_main = input.prefer_main
    if (input.background_lock_min !== null && input.background_lock_min !== undefined)
      prefs.background_lock_min = input.background_lock_min
    if (input.enabled) state.since = Date.now()
    if (input.device_name !== null && input.device_name !== undefined)
      mockAppSettings.device_name = input.device_name.trim() || null
    // The scripted phone shows up once sync is on and it is not paired yet.
    state.phase = prefs.enabled ? (state.peers.length ? 'live' : 'offer') : null
    push()
    return resolve(status())
  },
  sync_pair: ({ args }) => {
    // From the device list: the demo laptop is dialed, the way back is taken.
    if (args?.deviceId === TABLET.device_id && state.tablet) {
      state.tablet = { ...state.tablet, forgotten: false, online: true, last_synced_at: new Date().toISOString() }
      return resolve(null)
    }
    if (args?.deviceId && state.laptop) {
      state.laptop = { ...state.laptop, cut_by: null, online: true, last_synced_at: new Date().toISOString() }
      return resolve(null)
    }
    state.phase = 'pairing'
    state.pairingStart = Date.now()
    push()
    return resolve(null)
  },
  sync_dismiss_offer: () => {
    state.phase = null
    push()
    return resolve(null)
  },
  sync_confirm_pairing: ({ body }) => {
    const accept = Boolean((body as SyncDecisionInput).accept)
    if (accept) {
      state.peers = [
        {
          ...PHONE,
          paired_at: new Date().toISOString(),
          last_synced_at: null,
          online: true,
          paused: false,
          cut_by: null,
          forgotten: false,
        },
      ]
      state.phase = 'awaiting_confirm'
    } else state.phase = null
    push()
    return resolve(null)
  },
  sync_decide_plan: ({ body }) => {
    const accept = Boolean((body as SyncDecisionInput).accept)
    state.phase = accept ? 'live' : null
    // Declining pauses exchange with the device until "Resume".
    if (!accept) state.peers = state.peers.map((peer) => ({ ...peer, paused: true }))
    const batchId = `batch-${state.history.length + 1}`
    state.journal = [journalLine(accept ? 'applied' : 'declined', accept ? batchId : null), ...state.journal]
    if (accept) {
      const now = new Date().toISOString()
      state.history = [
        {
          id: batchId,
          skipped: [],
          peer_name: PHONE.name,
          applied_at: now,
          summary,
          undone_at: null,
          transferred_to: null,
        },
        ...state.history,
      ]
      state.peers = state.peers.map((peer) => ({ ...peer, last_synced_at: now }))
    }
    push()
    return resolve(null)
  },
  sync_peers: () =>
    resolve([
      ...state.peers.map((peer) => ({ ...peer, online: state.phase === 'live' })),
      ...(state.laptop && mockAppSettings.sync.enabled ? [state.laptop] : []),
      ...(state.tablet && mockAppSettings.sync.enabled ? [state.tablet] : []),
    ]),
  sync_forget_peer: ({ args }) => {
    if (args?.deviceId === TABLET.device_id) {
      state.tablet = state.tablet && { ...state.tablet, forgotten: true, online: false }
      return resolve(null)
    }
    if (args?.deviceId === LAPTOP.device_id) {
      state.laptop = null
      return resolve(null)
    }
    state.peers = []
    state.phase = null
    push()
    return resolve(null)
  },
  sync_resume_peer: ({ args }) => {
    const id = String(args?.deviceId ?? '')
    state.peers = state.peers.map((peer) => (peer.device_id === id ? { ...peer, paused: false } : peer))
    state.phase = 'live'
    push()
    return resolve(null)
  },
  sync_history: () => resolve(state.history),
  sync_journal: () => resolve(state.journal),
  sync_undo: ({ args }) => {
    const id = String(args?.batchId ?? '')
    state.history = state.history.map((batch) =>
      batch.id === id ? { ...batch, undone_at: new Date().toISOString() } : batch,
    )
    return resolve<UndoReport>({ restored: 3, skipped: ['Аренда'] })
  },
  // The mock never undoes a merge, so no record carries the mark; reminders are cleared for completeness.
  allow_merging: ({ args }) => {
    const input = args?.input as AllowMergingInput | undefined
    const reminder = input?.target === 'reminder' ? reminders.find((entry) => entry.id === input.id) : undefined
    if (reminder) reminder.keep = false
    return resolve(null)
  },
  sync_restore_backup: () => resolve(null),
  sync_open_firewall: () => {
    state.firewallOpen = true
    return resolve(status())
  },
}
