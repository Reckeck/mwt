import { type Ref, createApp, defineComponent, h, nextTick, ref } from 'vue'
import { VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia } from 'pinia'
import { describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'
import { i18n } from '@/shared/config/i18n'

import { useAdjustment } from './use-adjustment'

const settle = async () => {
  for (let tick = 0; tick < 5; tick++) await new Promise((resolve) => setTimeout(resolve, 0))
}

function mountAdjustment(transaction: Transaction | null) {
  const open = ref(true)
  let api!: ReturnType<typeof useAdjustment>
  const host = defineComponent({
    setup() {
      api = useAdjustment({ storeId: 'cash-1', transaction }, open as Ref<boolean>)
      return () => h('div')
    },
  })
  const app = createApp(host).use(i18n).use(createPinia()).use(VueQueryPlugin)
  app.mount(document.createElement('div'))
  return { api, open }
}

const argsOf = <Args>(command: string): Args => {
  const found = vi.mocked(call).mock.calls.find(([cmd]) => cmd === command)
  if (!found) throw new Error(`${command} was not called`)
  return found[1] as Args
}

const sentInput = () => argsOf<{ input: { common: { timestamp: string | null } } }>('add_transaction').input

describe('useAdjustment moment', () => {
  it('sends a null timestamp for a new correction until the date is touched', async () => {
    vi.mocked(call).mockImplementation(async (cmd: string) => {
      if (cmd === 'get_adjustment_draft') return { lines: [] } as never
      if (cmd === 'add_transaction') return {} as never
      throw new Error(`unmocked ${cmd}`)
    })
    const { api } = mountAdjustment(null)
    await settle()
    expect(api.timestamp.value).toBe('')
    api.rows.value = [{ asset: { kind: 'fiat', symbol: 'USD' } as unknown as AssetRef, before: '0', actual: '5' }]
    api.save.mutate()
    await settle()
    expect(sentInput().common.timestamp).toBeNull()
    expect(argsOf<{ input: { at: string | null } }>('get_adjustment_draft').input.at).toBeNull()
  })

  it('sends the ISO of the date once the user sets it', async () => {
    vi.mocked(call).mockClear()
    const { api } = mountAdjustment(null)
    await settle()
    api.timestamp.value = '2026-11-17T12:34'
    api.rows.value = [{ asset: { kind: 'fiat', symbol: 'USD' } as unknown as AssetRef, before: '0', actual: '5' }]
    api.save.mutate()
    await settle()
    expect(sentInput().common.timestamp).toBe(new Date('2026-11-17T12:34').toISOString())
  })

  it('keeps the original seconds of an edited correction', async () => {
    vi.mocked(call).mockClear()
    const original = '2026-11-17T12:34:56.000Z'
    const transaction = {
      adjustment: { meta: { id: 't1', timestamp: original, comment: '' }, lines: [] },
    } as unknown as Transaction
    const { api } = mountAdjustment(transaction)
    await settle()
    const saved = vi.mocked(call)
    saved.mockImplementation(async () => ({}) as never)
    api.save.mutate()
    await nextTick()
    await settle()
    const sent = argsOf<{ input: { transaction: { common: { timestamp: string } } } }>('update_transaction').input
    expect(sent.transaction.common.timestamp).toBe(original)
  })
})
