import { VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { describe, expect, it, vi } from 'vitest'
import { defineComponent } from 'vue'

import { call } from '@/shared/api/ipc'
import { useVaultStore } from '@/shared/store/vault'

import { useManualInflation } from './use-manual-inflation'

const row = (currency: string, manual: string | null) => ({ currency, manual, mode: 'manual' }) as InflationCurrency

/** The hook inside a host component, with an unlocked vault answering `rows`. */
async function setup(rows: InflationCurrency[]) {
  const pinia = createPinia()
  const answers = vi.mocked(call)
  answers.mockReset()
  answers.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_inflation_overview') return rows
    if (cmd === 'update_settings') return {}
    throw new Error(`unmocked ipc call: ${cmd}`)
  })
  let hook!: ReturnType<typeof useManualInflation>
  const Host = defineComponent({
    setup() {
      hook = useManualInflation()
      return () => null
    },
  })
  mount(Host, { global: { plugins: [pinia, VueQueryPlugin] } })
  useVaultStore(pinia).locked = false
  await flushPromises()
  return { hook, answers }
}

describe('useManualInflation', () => {
  it('reads the group table from the overview, not from the app settings', async () => {
    const { hook } = await setup([row('rub', '8'), row('gbp', '4'), row('chf', null)])
    expect(hook.table.value).toEqual({ rub: '8', gbp: '4' })
  })

  it('saves the whole table through update_settings as inflation_manual', async () => {
    const { hook, answers } = await setup([row('rub', '8'), row('gbp', '4')])
    hook.table.value = { ...hook.table.value, gbp: '4,5' }
    await hook.persist(true)
    const saved = answers.mock.calls.filter(([cmd]) => cmd === 'update_settings')
    expect(saved).toHaveLength(1)
    expect(saved[0]?.[1]).toMatchObject({ input: { inflation_manual: { rub: '8', gbp: '4.5' } } })
    expect(answers.mock.calls.some(([cmd]) => cmd === 'update_app_settings')).toBe(false)
  })
})
