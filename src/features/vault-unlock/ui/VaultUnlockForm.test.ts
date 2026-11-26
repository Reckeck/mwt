import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'

import VaultUnlockForm from './VaultUnlockForm.vue'

vi.mock('vue-router', () => ({ useRouter: () => ({ replace: vi.fn(async () => undefined) }) }))

const mounted: ReturnType<typeof mount>[] = []

beforeEach(() => {
  setActivePinia(createPinia())
  vi.mocked(call).mockImplementation(async (cmd: string) => {
    if (cmd === 'unlock_vault') throw { code: 'PIN_RETRY_LATER', message: 'wait 30 s' }
    throw new Error(`unmocked ipc call: ${cmd}`)
  })
})

afterEach(() => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
})

describe('VaultUnlockForm', () => {
  it('shows the PIN_RETRY_LATER text as the unlock error, with no countdown', async () => {
    const wrapper = mount(VaultUnlockForm, {
      props: { path: '/tmp/mock/v.wallet', plain: false },
      global: { stubs: { BiometricUnlockButton: true } },
    })
    mounted.push(wrapper)
    await wrapper.get('#unlock-card-password').setValue('123456')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    const shown = wrapper.get('[data-unlock-error]').text()
    expect(shown).toContain('Слишком много неверных PIN')
    expect(shown).not.toContain('wait 30 s')
  })
})
