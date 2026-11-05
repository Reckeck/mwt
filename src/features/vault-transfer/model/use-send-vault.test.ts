import { flushPromises } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { createApp, defineComponent, h } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'

import { useSendVault } from './use-send-vault'

const input = { device_id: 'd1', password: 'pw' } as unknown as TransferSendInput
const waiting = { phase: 'waiting', device_name: 'Phone' } as unknown as TransferSendView

function mountHook() {
  let api!: ReturnType<typeof useSendVault>
  const app = createApp(
    defineComponent({
      setup() {
        api = useSendVault()
        return () => h('div')
      },
    }),
  )
  app.use(VueQueryPlugin, { queryClient: new QueryClient() })
  app.mount(document.createElement('div'))
  return api
}

describe('useSendVault', () => {
  beforeEach(() => {
    vi.mocked(call).mockReset()
  })

  it('sets the view when a send finishes with the dialog open', async () => {
    vi.mocked(call).mockResolvedValue(waiting)
    const api = mountHook()
    api.send.mutate(input)
    await flushPromises()
    expect(api.view.value).toEqual(waiting)
  })

  it('cancels and sets no view when the dialog closed during the password check', async () => {
    let finish!: (view: TransferSendView) => void
    vi.mocked(call).mockImplementation(((cmd: string) =>
      cmd === 'transfer_send'
        ? new Promise((resolve) => {
            finish = resolve as (view: TransferSendView) => void
          })
        : Promise.resolve(undefined)) as typeof call)
    const api = mountHook()
    api.send.mutate(input)
    await flushPromises()
    api.close()
    api.send.reset()
    vi.mocked(call).mockClear()
    finish(waiting)
    await flushPromises()
    expect(api.view.value).toBeNull()
    expect(call).toHaveBeenCalledWith('transfer_send_cancel')
  })
})
