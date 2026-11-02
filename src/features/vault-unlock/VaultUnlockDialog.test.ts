import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'
import { useVaultStore } from '@/shared/store/vault'

import VaultUnlockDialog from './VaultUnlockDialog.vue'

const replace = vi.hoisted(() => vi.fn(async () => undefined))
vi.mock('vue-router', () => ({ useRouter: () => ({ replace }) }))

const mounted: ReturnType<typeof mount>[] = []

beforeEach(() => {
  setActivePinia(createPinia())
  vi.mocked(call).mockImplementation(async (cmd: string) => {
    throw new Error(`unmocked ipc call: ${cmd}`)
  })
})

afterEach(() => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
  document.body.innerHTML = ''
})

describe('VaultUnlockDialog', () => {
  it('shows the given path when it mounts already open (a received vault)', async () => {
    useVaultStore()
    const wrapper = mount(VaultUnlockDialog, {
      attachTo: document.body,
      props: { open: true, path: '/tmp/mock/received.wallet' },
    })
    mounted.push(wrapper)
    await flushPromises()
    expect(document.body.textContent).toContain('/tmp/mock/received.wallet')
    expect(document.body.querySelector('#unlock-path')).toBeNull()
  })

  it('starts over for another path when it is reopened', async () => {
    useVaultStore()
    const wrapper = mount(VaultUnlockDialog, { attachTo: document.body, props: { open: false, path: '/a.wallet' } })
    mounted.push(wrapper)
    await wrapper.setProps({ path: '/b.wallet', open: true })
    await flushPromises()
    expect(document.body.textContent).toContain('/b.wallet')
  })
})
