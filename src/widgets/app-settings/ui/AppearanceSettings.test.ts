import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { appSettingsOutsideCommands } from '@/shared/api/mock/commands-app'
import { mockAppSettings } from '@/shared/api/mock/vault-data'
import { useSettingsStore } from '@/shared/store/settings'

import AppearanceSettings from './AppearanceSettings.vue'

vi.mock('@/shared/api/ipc', () => ({
  call: vi.fn(async (cmd: string, args?: { input?: unknown }) => {
    const handler = appSettingsOutsideCommands[cmd]
    if (!handler) throw new Error(`unmocked ipc call: ${cmd}`)
    return handler({ cmd, args, body: args?.input } as never)
  }),
}))

const flush = () => new Promise((resolve) => setTimeout(resolve))

async function mountModal(settings: Record<string, unknown>) {
  setActivePinia(createPinia())
  Object.assign(mockAppSettings, { theme_id: null, theme_light_id: null, theme_dark_id: null }, settings)
  const store = useSettingsStore()
  await store.loadApp()
  const wrapper = mount(AppearanceSettings, {
    attachTo: document.body,
    // The host persists the patch; here the store stands in for it.
    props: { onChange: (patch: Partial<UpdateAppSettingsInput>) => void store.updateApp(patch) },
  })
  await flush()
  return { wrapper, store }
}

const checkedId = (root: { find: (selector: string) => { attributes: (name: string) => string | undefined } }) =>
  root.find('[aria-checked="true"]').attributes('data-theme-id')

describe('Appearance settings', () => {
  beforeEach(() => {
    document.body.innerHTML = ''
  })

  it('shows Default selected for a user with no ids (TP8)', async () => {
    const { wrapper } = await mountModal({ theme: 'light' })
    expect(checkedId(wrapper)).toBe('default-light')
  })

  it('under Dark shows one picker of dark themes and persists themeId at once', async () => {
    const { wrapper, store } = await mountModal({ theme: 'dark' })
    const radios = wrapper.findAll('[role="radio"]')
    expect(wrapper.findAll('[role="radiogroup"]')).toHaveLength(1)
    expect(radios.every((radio) => radio.attributes('data-theme-id')?.length)).toBe(true)
    await wrapper.find('[data-theme-id="dracula"]').trigger('click')
    await vi.waitFor(() => expect(store.themeId).toBe('dracula'))
    expect(checkedId(wrapper)).toBe('dracula')
  })

  it('under Auto two pickers persist the light and the dark slot, keeping themeId', async () => {
    const { wrapper, store } = await mountModal({ theme: 'auto', theme_id: 'nord' })
    expect(wrapper.findAll('[role="radiogroup"]')).toHaveLength(2)
    const lightId = wrapper
      .find('[data-slot="light"] [role="radio"]:not([aria-checked="true"])')
      .attributes('data-theme-id')
    const darkId = wrapper
      .find('[data-slot="dark"] [role="radio"]:not([aria-checked="true"])')
      .attributes('data-theme-id')
    await wrapper.find(`[data-slot="light"] [data-theme-id="${lightId}"]`).trigger('click')
    await wrapper.find(`[data-slot="dark"] [data-theme-id="${darkId}"]`).trigger('click')
    await vi.waitFor(() => expect(store.themeDarkId).toBe(darkId))
    expect(store.themeLightId).toBe(lightId)
    expect(store.themeId).toBe('nord')
  })
})
