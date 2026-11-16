import { describe, expect, it, vi } from 'vitest'
import { computed, ref } from 'vue'

const query = ref<Record<string, string>>({})
vi.mock('vue-router', () => ({
  useRoute: () => ({ query: query.value, params: { id: 'w1' } }),
  useRouter: () => ({ replace: vi.fn() }),
}))
vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('@/shared/lib/phone', () => ({ useIsPhone: () => ref(false) }))

import { useWalletTabs } from './use-wallet-view'

function tabsOf(kind: 'home' | 'money' | 'coins' | 'exchange', engine: boolean) {
  return useWalletTabs(
    ref(kind),
    ref(false),
    computed(() => engine),
  )
}

describe('the relations tab', () => {
  it('belongs only to a wallet the engine drives', () => {
    for (const kind of ['home', 'money'] as const) {
      const { tabs, more } = tabsOf(kind, false)
      expect([...tabs.value, ...more.value].map((tab) => tab.value)).not.toContain('relations')
    }
    expect(tabsOf('coins', false).tabs.value.map((tab) => tab.value)).not.toContain('relations')
    expect(tabsOf('coins', true).tabs.value.map((tab) => tab.value)).toContain('relations')
    expect(tabsOf('exchange', true).more.value.map((tab) => tab.value)).toContain('relations')
  })

  it('an old ?tab=relations link opens the overview of a wallet without the tab', () => {
    query.value = { tab: 'relations' }
    expect(tabsOf('money', false).tab.value).toBe('history')
    expect(tabsOf('coins', false).tab.value).toBe('positions')
    expect(tabsOf('coins', true).tab.value).toBe('relations')
    query.value = {}
  })
})
