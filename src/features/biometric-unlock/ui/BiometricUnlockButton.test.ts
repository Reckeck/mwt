import { flushPromises, mount } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import * as ipc from '@/shared/api/ipc'

import BiometricUnlockButton from './BiometricUnlockButton.vue'

vi.mock('vue-router', () => ({ useRouter: () => ({ replace: vi.fn() }) }))

function mountWith(state: string, unlock?: () => Promise<unknown>) {
  const pinia = createPinia()
  setActivePinia(pinia)
  vi.spyOn(ipc, 'call').mockImplementation((async (cmd: string) => {
    if (cmd === 'biometric_state') return state
    if (cmd === 'biometric_unlock') return unlock?.()
    return null
  }) as never)
  return mount(BiometricUnlockButton, {
    props: { path: '/a.wallet' },
    global: { plugins: [pinia, [VueQueryPlugin, { queryClient: new QueryClient() }]] },
  })
}

describe('BiometricUnlockButton', () => {
  beforeEach(() => vi.restoreAllMocks())

  it('draws nothing where the platform has no binding', async () => {
    const wrapper = mountWith('unavailable')
    await flushPromises()
    expect(wrapper.find('[data-biometric-unlock]').exists()).toBe(false)
  })

  it('offers the button when quick sign-in is on', async () => {
    const wrapper = mountWith('enabled')
    await flushPromises()
    expect(wrapper.find('button').attributes('disabled')).toBeUndefined()
  })

  it('disables the button and asks for the password once the 7 days are over', async () => {
    const wrapper = mountWith('password_required')
    await flushPromises()
    expect(wrapper.find('button').attributes('disabled')).toBeDefined()
    expect(wrapper.find('[role="alert"]').exists()).toBe(true)
  })

  it('shows the reset notice after the binding was dropped', async () => {
    const wrapper = mountWith('enabled', () => Promise.reject('BIOMETRIC_INVALIDATED: reset'))
    await flushPromises()
    await wrapper.find('button').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('Биометрия выключена')
  })
})
