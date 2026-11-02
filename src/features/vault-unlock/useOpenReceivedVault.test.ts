import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent } from 'vue'
import { mount } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { useOpenReceivedVault } from './useOpenReceivedVault'

const replace = vi.hoisted(() => vi.fn(async () => undefined))
vi.mock('vue-router', () => ({ useRouter: () => ({ replace }) }))

const PATH = '/tmp/mock/received.wallet'
const info = { path: PATH, encrypted: false }

function backend(encrypted: boolean) {
  vi.mocked(call).mockImplementation(async (cmd: string) => {
    if (cmd === 'recent_vaults') return [{ path: PATH, encrypted, exists: true, display_name: 'received.wallet' }]
    if (cmd === 'unlock_vault') return info
    if (cmd === 'lock_vault_cmd') return null
    if (cmd === 'vault_status') return { path: PATH, exists: true, unlocked: false, encrypted }
    throw new Error(`unmocked ipc call: ${cmd}`)
  })
}

function setup() {
  let api!: ReturnType<typeof useOpenReceivedVault>
  mount(
    defineComponent({
      setup() {
        api = useOpenReceivedVault()
        return () => null
      },
    }),
    { global: { plugins: [[VueQueryPlugin, { queryClient: new QueryClient() }]] } },
  )
  return api
}

beforeEach(() => {
  setActivePinia(createPinia())
  replace.mockClear()
  vi.mocked(call).mockClear()
})

describe('useOpenReceivedVault', () => {
  it('opens a vault without a password right away', async () => {
    backend(false)
    const opened = await setup().openIfPlain(PATH)
    expect(opened).toBe(true)
    expect(call).toHaveBeenCalledWith('unlock_vault', { input: { path: PATH, password: '' } })
    expect(replace).toHaveBeenCalledWith({ name: 'dashboard' })
  })

  it('leaves an encrypted vault to the dialog', async () => {
    backend(true)
    const opened = await setup().openIfPlain(PATH)
    expect(opened).toBe(false)
    expect(call).not.toHaveBeenCalledWith('unlock_vault', expect.anything())
  })

  it('locks the open vault and sends an encrypted one to the unlock screen', async () => {
    backend(true)
    await setup().switchTo(PATH)
    expect(call).toHaveBeenCalledWith('lock_vault_cmd')
    expect(replace).toHaveBeenCalledWith({ name: 'unlock', query: { received: PATH } })
  })

  it('switches to a plain vault without locking', async () => {
    backend(false)
    await setup().switchTo(PATH)
    expect(call).not.toHaveBeenCalledWith('lock_vault_cmd')
    expect(call).toHaveBeenCalledWith('unlock_vault', { input: { path: PATH, password: '' } })
  })
})
