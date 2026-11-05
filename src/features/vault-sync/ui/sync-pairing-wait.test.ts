import { flushPromises, mount } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { computed, ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'

import { SyncPairingDialog } from './SyncPairingDialog'

const pairingSession = ref<unknown>(null)

vi.mock('../model/use-sync-status', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../model/use-sync-status')>()),
  useSyncStatus: () => ({ pairing: computed(() => pairingSession.value) }),
}))

const WAITING = 'Ждём подтверждения на втором устройстве'
const text = () => document.body.textContent ?? ''
const button = (label: string) =>
  [...document.body.querySelectorAll('button')].find((node) => node.textContent?.includes(label))
const session = (id: string, name: string) => ({
  session_id: id,
  name,
  phase: 'pairing',
  pairing_code: '123456',
  pairing_expires_ms: 0,
})

async function open() {
  const wrapper = mount(SyncPairingDialog, {
    attachTo: document.body,
    global: { plugins: [[VueQueryPlugin, { queryClient: new QueryClient() }]] },
  })
  await flushPromises()
  return wrapper
}

describe('pairing dialog waiting state', () => {
  beforeEach(() => {
    vi.mocked(call).mockReset()
    vi.mocked(call).mockResolvedValue(null)
    pairingSession.value = session('p1', 'Phone')
  })
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('stays open after Done and shows it waits, with Done disabled', async () => {
    const wrapper = await open()
    button('Готово')?.click()
    await flushPromises()
    expect(text()).toContain('Добавить устройство')
    expect(text()).toContain(WAITING)
    expect(button('Готово')?.disabled).toBe(true)
    wrapper.unmount()
  })

  it('Cancel while waiting declines and hides the dialog', async () => {
    const wrapper = await open()
    button('Готово')?.click()
    await flushPromises()
    button('Отмена')?.click()
    await flushPromises()
    expect(call).toHaveBeenLastCalledWith('sync_confirm_pairing', { input: { session_id: 'p1', accept: false } })
    expect(text()).not.toContain('Добавить устройство')
    wrapper.unmount()
  })

  it('a decline hides the dialog at once', async () => {
    const wrapper = await open()
    button('Отмена')?.click()
    await flushPromises()
    expect(text()).not.toContain('Добавить устройство')
    wrapper.unmount()
  })

  it('shows a failed accept as the normal state again', async () => {
    vi.mocked(call).mockRejectedValue(new Error('IO: broken'))
    const wrapper = await open()
    button('Готово')?.click()
    await flushPromises()
    expect(text()).toContain('Добавить устройство')
    expect(text()).not.toContain(WAITING)
    expect(button('Готово')?.disabled).toBe(false)
    wrapper.unmount()
  })

  it('shows the second pairing once the first is gone', async () => {
    const wrapper = await open()
    button('Готово')?.click()
    await flushPromises()
    pairingSession.value = session('p2', 'Tablet')
    await flushPromises()
    expect(text()).toContain('Tablet')
    expect(text()).not.toContain(WAITING)
    wrapper.unmount()
  })
})
