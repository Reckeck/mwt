import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { defineComponent } from 'vue'
import { mount } from '@vue/test-utils'

import { i18n } from '@/shared/config/i18n'

import { useProfileMenu } from './use-profile-menu'

vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn(), currentRoute: { value: { query: {} } } }) }))

function setup() {
  let api!: ReturnType<typeof useProfileMenu>
  mount(
    defineComponent({
      setup() {
        api = useProfileMenu({ tour: () => ({ available: true, steps: 3 }), startTour: vi.fn(), allTours: vi.fn() })
        return () => null
      },
    }),
    { global: { plugins: [[VueQueryPlugin, { queryClient: new QueryClient() }]] } },
  )
  return api
}

describe('useProfileMenu', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    i18n.global.locale.value = 'ru'
  })

  it('relabels the vault rows when the locale changes', () => {
    const menu = setup()
    const labelsOf = () => menu.vaultEntries.value.map((entry) => `${entry.label}|${entry.hint ?? ''}`)
    const russian = labelsOf()
    expect(russian.length).toBeGreaterThan(0)
    i18n.global.locale.value = 'en'
    expect(labelsOf()).not.toEqual(russian)
    i18n.global.locale.value = 'ru'
  })

  it('holds the notifications and tour rows that left the top bar', () => {
    const menu = setup()
    const ids = menu.statusEntries.value.map((entry) => entry.id)
    expect(ids).toEqual(expect.arrayContaining(['notifications', 'tour', 'tours']))
    expect(menu.attention.value).toBe(false)
  })

  it('drops the screen tour row when the screen has none', () => {
    let api!: ReturnType<typeof useProfileMenu>
    mount(
      defineComponent({
        setup() {
          api = useProfileMenu({ tour: () => ({ available: false, steps: 0 }), startTour: vi.fn(), allTours: vi.fn() })
          return () => null
        },
      }),
      { global: { plugins: [[VueQueryPlugin, { queryClient: new QueryClient() }]] } },
    )
    expect(api.statusEntries.value.map((entry) => entry.id)).not.toContain('tour')
  })
})
