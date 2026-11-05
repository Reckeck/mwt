import { flushPromises } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia } from 'pinia'
import { createApp, defineComponent, h } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { call, onEvent } from '@/shared/api/ipc'
import { i18n } from '@/shared/config/i18n'
import { notify } from '@/shared/lib/notify'

import { useSyncEvents } from './use-sync-events'
import { SYNC_HISTORY_KEY, SYNC_JOURNAL_KEY, SYNC_PEERS_KEY } from './use-sync-status'

vi.mock('@/shared/lib/notify', () => ({
  notify: { success: vi.fn(), info: vi.fn(), error: vi.fn(), warning: vi.fn() },
}))

const handlers = new Map<string, (payload: unknown) => void>()

function mountEvents() {
  const queryClient = new QueryClient()
  const spy = vi.spyOn(queryClient, 'invalidateQueries')
  const app = createApp(
    defineComponent({
      setup() {
        useSyncEvents()
        return () => h('div')
      },
    }),
  )
  app.use(createPinia())
  app.use(i18n)
  app.use(VueQueryPlugin, { queryClient })
  app.mount(document.createElement('div'))
  return spy
}

const synced = (total: number, collections: string[] = []) => ({
  session_id: 's1',
  collections,
  batch: { id: 'b1', peer_name: 'Phone', summary: { total, collections: [], overwritten: [], unstamped_conflicts: 0 } },
})

const keysInvalidated = (spy: ReturnType<typeof mountEvents>) =>
  spy.mock.calls.map(([filters]) => JSON.stringify((filters as { queryKey?: unknown } | undefined)?.queryKey))

describe('useSyncEvents: vault-synced', () => {
  beforeEach(() => {
    handlers.clear()
    vi.mocked(call).mockReset()
    vi.mocked(notify.success).mockReset()
    vi.mocked(notify.info).mockReset()
    vi.mocked(onEvent).mockImplementation((async (name: string, handler: (payload: unknown) => void) => {
      handlers.set(name, handler)
      return () => undefined
    }) as unknown as typeof onEvent)
  })

  it('refreshes history and peers as well as the journal', () => {
    const spy = mountEvents()
    handlers.get('vault-synced')?.(synced(3))
    const keys = keysInvalidated(spy)
    expect(keys).toContain(JSON.stringify(SYNC_HISTORY_KEY))
    expect(keys).toContain(JSON.stringify(SYNC_PEERS_KEY))
    expect(keys).toContain(JSON.stringify(SYNC_JOURNAL_KEY))
  })

  it('shows no toast but still refreshes for a stamp-only batch', () => {
    const spy = mountEvents()
    handlers.get('vault-synced')?.(synced(0))
    expect(notify.success).not.toHaveBeenCalled()
    expect(keysInvalidated(spy)).toContain(JSON.stringify(SYNC_HISTORY_KEY))
  })

  it('undoes through useSyncActions: no manual notification', async () => {
    vi.mocked(call).mockResolvedValue({ restored: 2, skipped: ['a'] })
    mountEvents()
    handlers.get('vault-synced')?.(synced(2))
    const options = vi.mocked(notify.success).mock.calls[0]?.[1] as { action: { onClick: () => void } }
    options.action.onClick()
    await flushPromises()
    expect(call).toHaveBeenCalledWith('sync_undo', { batchId: 'b1' })
    expect(notify.info).not.toHaveBeenCalled()
  })

  it('refreshes the screens of a collection only the event names (dedupe and repair deletions)', () => {
    const spy = mountEvents()
    handlers.get('vault-synced')?.(synced(0, ['transactions']))
    // A ledger collection: the whole app re-reads (an unfiltered invalidate).
    expect(spy.mock.calls.some(([filters]) => filters === undefined)).toBe(true)
  })

  it('refreshes the own queries of a non-ledger collection in `collections`', () => {
    const spy = mountEvents()
    handlers.get('vault-synced')?.(synced(0, ['contacts']))
    expect(keysInvalidated(spy)).toContain(JSON.stringify(['contacts']))
    expect(spy.mock.calls.some(([filters]) => filters === undefined)).toBe(false)
  })
})
