import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { mockAppSettings } from '@/shared/api/mock/vault-data'
import { appSettingsOutsideCommands } from '@/shared/api/mock/commands-app'

import { useSettingsStore } from './settings'

vi.mock('@/shared/api/ipc', () => ({
  call: vi.fn(async (cmd: string, args?: { input?: unknown }) => {
    const handler = appSettingsOutsideCommands[cmd]
    if (!handler) throw new Error(`unmocked ipc call: ${cmd}`)
    return handler({ cmd, args, body: args?.input } as never)
  }),
}))

describe('settings store theme ids', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    Object.assign(mockAppSettings, {
      theme: 'dark',
      theme_id: 'nord',
      theme_light_id: 'paper',
      theme_dark_id: 'night',
    })
  })

  it('reads the ids, null meaning Default', async () => {
    const store = useSettingsStore()
    expect(store.themeId).toBeNull()
    await store.loadApp()
    expect([store.themeId, store.themeLightId, store.themeDarkId]).toEqual(['nord', 'paper', 'night'])
  })

  it('patching one id leaves the other two and the mode unchanged', async () => {
    const store = useSettingsStore()
    await store.updateApp({ theme_light_id: 'solar' })
    expect(store.themeLightId).toBe('solar')
    expect(store.themeId).toBe('nord')
    expect(store.themeDarkId).toBe('night')
    expect(store.theme).toBe('dark')
  })

  it('an empty string clears an id back to Default', async () => {
    const store = useSettingsStore()
    await store.updateApp({ theme_id: '' })
    expect(store.themeId).toBeNull()
    expect(store.themeDarkId).toBe('night')
  })
})
