import { describe, expect, it, vi } from 'vitest'

import { call } from './ipc'
import { openSystemPicker } from './system-picker'

describe('openSystemPicker', () => {
  it('sends the hold and clicks the input in the same tick', () => {
    const order: string[] = []
    vi.mocked(call).mockImplementationOnce(async (cmd: string) => {
      order.push(cmd)
      return undefined as never
    })
    const input = document.createElement('input')
    vi.spyOn(input, 'click').mockImplementation(() => void order.push('click'))
    openSystemPicker(input)
    // no await between the call and the assertion: the click did not wait for the IPC
    expect(order).toEqual(['autolock_hold_for_picker', 'click'])
  })

  it('still opens the picker when the hold fails', async () => {
    vi.mocked(call).mockRejectedValueOnce(new Error('boom'))
    const input = document.createElement('input')
    const click = vi.spyOn(input, 'click')
    openSystemPicker(input)
    expect(click).toHaveBeenCalledOnce()
    await Promise.resolve()
  })
})
