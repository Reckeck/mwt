import { describe, expect, it, vi } from 'vitest'

import { listenMock } from '../mock-bus'
import * as sync from './commands-sync'

const run = (cmd: string, body?: unknown, args?: Record<string, unknown>) =>
  sync.syncCommands[cmd]?.({ cmd, args, body: body as Record<string, unknown> | undefined })
const status = async () => (await run('sync_status')) as SyncStatus
const setSettings = (input: Partial<SyncSettingsInput>) =>
  run('sync_set_settings', {
    enabled: null,
    confirm_first: null,
    prefer_main: null,
    device_name: null,
    background_lock_min: null,
    ...input,
  })

describe('sync mock', () => {
  it('role follows prefer_main, not always main', async () => {
    await setSettings({ enabled: true, prefer_main: false })
    expect((await status()).role).toBe('client')
    await setSettings({ prefer_main: true })
    expect((await status()).role).toBe('main')
    await setSettings({ enabled: false })
    expect((await status()).role).toBe('off')
  })

  it('exposes last_error and background_supported from a settable state', async () => {
    const set = (sync as unknown as { setMockSyncState?: (patch: object) => void }).setMockSyncState
    expect(set).toBeTypeOf('function')
    set?.({ lastError: 'IO', backgroundSupported: true })
    const view = await status()
    expect(view.network.last_error).toBe('IO')
    expect(view.background_supported).toBe(true)
    set?.({ lastError: null, backgroundSupported: false })
  })

  it('accepting a plan emits vault-synced with the applied batch', async () => {
    const seen = vi.fn()
    const stop = listenMock('vault-synced', seen)
    await setSettings({ enabled: true })
    await run('sync_pair', undefined, {})
    await run('sync_confirm_pairing', { session_id: 'mock-session', accept: true })
    await run('sync_decide_plan', { session_id: 'mock-session', accept: true })
    await new Promise((done) => setTimeout(done, 5))
    stop()
    expect(seen).toHaveBeenCalledTimes(1)
    const event = seen.mock.calls[0]?.[0] as VaultSyncedEvent
    expect(event.session_id).toBe('mock-session')
    expect(event.batch.summary.total).toBeGreaterThan(0)
    const history = (await run('sync_history')) as AppliedBatch[]
    expect(history[0]?.id).toBe(event.batch.id)
  })
})
