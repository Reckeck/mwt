import { flushPromises, mount } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { computed, ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'

import { SyncPairingDialog } from './SyncPairingDialog'
import { SyncPlanDialog } from './SyncPlanDialog'

const pairingSession = ref<unknown>(null)
const planSession = ref<unknown>(null)

vi.mock('../model/use-sync-status', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../model/use-sync-status')>()),
  useSyncStatus: () => ({ pairing: computed(() => pairingSession.value), plan: computed(() => planSession.value) }),
}))
vi.mock('vue-router', () => ({ useRoute: () => ({ query: {} }) }))

const text = () => document.body.textContent ?? ''
const press = (label: string) => {
  const button = [...document.body.querySelectorAll('button')].find((node) => node.textContent?.includes(label))
  button?.click()
}

function mountDialog(component: object) {
  return mount(component, {
    attachTo: document.body,
    global: { plugins: [[VueQueryPlugin, { queryClient: new QueryClient() }]] },
  })
}

describe('sync dialogs reopen after a failed answer', () => {
  beforeEach(() => {
    vi.mocked(call).mockReset()
    vi.mocked(call).mockRejectedValue(new Error('IO: broken'))
    pairingSession.value = {
      session_id: 'p1',
      name: 'Phone',
      phase: 'pairing',
      pairing_code: '123456',
      pairing_expires_ms: 0,
    }
    planSession.value = {
      session_id: 'a1',
      name: 'Laptop',
      phase: 'awaiting_confirm',
      plan: {
        reason: 'first',
        after: null,
        summary: { total: 2, collections: [], overwritten: [], unstamped_conflicts: 0 },
      },
    }
  })
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('pairing dialog is shown again when confirming fails', async () => {
    const wrapper = mountDialog(SyncPairingDialog)
    await flushPromises()
    expect(text()).toContain('Добавить устройство')
    press('Готово')
    await flushPromises()
    expect(call).toHaveBeenCalledWith('sync_confirm_pairing', expect.anything())
    expect(text()).toContain('Добавить устройство')
    wrapper.unmount()
  })

  it('plan dialog is shown again when applying fails', async () => {
    const wrapper = mountDialog(SyncPlanDialog)
    await flushPromises()
    expect(text()).toContain('Применить')
    press('Применить')
    await flushPromises()
    expect(call).toHaveBeenCalledWith('sync_decide_plan', expect.anything())
    expect(text()).toContain('Применить')
    wrapper.unmount()
  })
})
