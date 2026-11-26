import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, h } from 'vue'

import { call } from '@/shared/api/ipc'

import { useUnlockVault } from './useUnlockVault'

vi.mock('vue-router', () => ({ useRouter: () => ({ replace: vi.fn(async () => undefined) }) }))

function mountComposable() {
  let state!: ReturnType<typeof useUnlockVault>
  const wrapper = mount(
    defineComponent({
      setup() {
        state = useUnlockVault()
        return () => h('div')
      },
    }),
  )
  return { state, wrapper }
}

beforeEach(() => {
  setActivePinia(createPinia())
  vi.mocked(call).mockImplementation(async (cmd: string) => {
    // The retry after the resolve is not under test: let it fail and stop.
    if (cmd === 'resolve_vault_conflict') return undefined
    if (cmd === 'unlock_vault') throw new Error('stop after the retry')
    throw new Error(`unmocked ipc call: ${cmd}`)
  })
})

describe('useUnlockVault', () => {
  it('resolveConflict sends the path, the kept side and the password', async () => {
    const { state, wrapper } = mountComposable()
    state.reset('/tmp/mock/v.wallet')
    state.password.value = 'secret'
    await state.resolveConflict('local')
    await flushPromises()
    expect(call).toHaveBeenCalledWith('resolve_vault_conflict', {
      input: { path: '/tmp/mock/v.wallet', keep: 'local', password: 'secret' },
    })
    wrapper.unmount()
  })
})
