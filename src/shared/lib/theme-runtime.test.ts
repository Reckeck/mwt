import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, nextTick } from 'vue'

import { call } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

import { getTheme } from './theme/registry'
import { resolveTheme, useTheme } from './theme'

vi.mock('@/shared/api/ipc', () => ({ call: vi.fn().mockResolvedValue(undefined) }))

const modes = ['light', 'dark', 'auto'] as const
const none = { themeId: null, themeLightId: null, themeDarkId: null }

describe('resolveTheme: mode x system mode', () => {
  for (const preference of modes) {
    for (const systemDark of [false, true]) {
      it(`${preference} with the system ${systemDark ? 'dark' : 'light'} falls back to Default`, () => {
        const expectedMode = preference === 'auto' ? (systemDark ? 'dark' : 'light') : preference
        expect(resolveTheme({ preference, ...none }, systemDark).id).toBe(`default-${expectedMode}`)
      })
    }
  }

  it('Light and Dark use themeId of that mode and ignore the system mode', () => {
    expect(resolveTheme({ preference: 'dark', ...none, themeId: 'dracula' }, false).id).toBe('dracula')
    expect(resolveTheme({ preference: 'light', ...none, themeId: 'nord-snow-storm' }, true).id).toBe('nord-snow-storm')
  })

  it('a stored id of the other mode falls back to Default of the mode', () => {
    expect(resolveTheme({ preference: 'light', ...none, themeId: 'dracula' }, true).id).toBe('default-light')
  })

  it('a stale or removed id falls back to Default without throwing', () => {
    expect(resolveTheme({ preference: 'dark', ...none, themeId: 'gone' }, false).id).toBe('default-dark')
    expect(resolveTheme({ preference: 'auto', ...none, themeLightId: 'gone', themeDarkId: 'gone' }, true).id).toBe(
      'default-dark',
    )
  })

  it('Auto picks the light or the dark slot by the system mode', () => {
    const pair = { preference: 'auto' as const, themeId: null, themeLightId: 'nord-snow-storm', themeDarkId: 'dracula' }
    expect(resolveTheme(pair, false).id).toBe('nord-snow-storm')
    expect(resolveTheme(pair, true).id).toBe('dracula')
  })
})

describe('useTheme', () => {
  let listener: ((event: { matches: boolean }) => void) | undefined

  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    listener = undefined
    vi.stubGlobal('matchMedia', () => ({
      matches: false,
      addEventListener: (_name: string, handler: typeof listener) => {
        listener = handler
      },
      removeEventListener: vi.fn(),
    }))
  })
  afterEach(() => {
    vi.unstubAllGlobals()
    document.documentElement.removeAttribute('data-theme')
  })

  it('switches the Auto pair live on the media event and mirrors the choice', async () => {
    const store = useSettingsStore()
    store.app = {
      theme: 'auto',
      theme_id: null,
      theme_light_id: 'nord-snow-storm',
      theme_dark_id: 'dracula',
    } as never
    const wrapper = mount(
      defineComponent({
        setup() {
          useTheme()
          return () => null
        },
      }),
    )
    const root = document.documentElement
    expect(root.getAttribute('data-theme')).toBe('light')
    listener?.({ matches: true })
    await nextTick()
    expect(root.getAttribute('data-theme')).toBe('dark')
    expect(localStorage.getItem('trade-app:theme')).toBe('auto')
    expect(localStorage.getItem('trade-app:theme-light-id')).toBe('nord-snow-storm')
    expect(localStorage.getItem('trade-app:theme-dark-id')).toBe('dracula')
    wrapper.unmount()
  })

  it('paints the window with the resolved --background on every apply and on an Auto system switch', async () => {
    const store = useSettingsStore()
    store.app = { theme: 'auto', theme_id: null, theme_light_id: 'nord-snow-storm', theme_dark_id: 'dracula' } as never
    vi.mocked(call).mockClear()
    const wrapper = mount(
      defineComponent({
        setup() {
          useTheme()
          return () => null
        },
      }),
    )
    const light = getTheme('nord-snow-storm')?.tokens['--background']
    const dark = getTheme('dracula')?.tokens['--background']
    expect(call).toHaveBeenLastCalledWith('set_window_background', { rgb: light?.toLowerCase() })
    listener?.({ matches: true })
    await nextTick()
    expect(call).toHaveBeenLastCalledWith('set_window_background', { rgb: dark?.toLowerCase() })
    store.app = { ...(store.app as object), theme: 'light', theme_id: 'nord-snow-storm' } as never
    await nextTick()
    expect(call).toHaveBeenLastCalledWith('set_window_background', { rgb: light?.toLowerCase() })
    wrapper.unmount()
  })
})
