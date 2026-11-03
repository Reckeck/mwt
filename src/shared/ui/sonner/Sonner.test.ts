import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import { toast } from 'vue-sonner'
import { Sonner } from '.'

const EVENT = 'visibilitychange'

describe('Sonner', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  // vue-sonner 2.0.9 added a document listener per toast and never removed it (it returned the
  // cleanup from watchEffect, which Vue ignores); patches/vue-sonner@2.0.9.patch fixes it.
  it('removes every listener a toast adds once the toasts are gone', async () => {
    const added = vi.spyOn(document, 'addEventListener')
    const removed = vi.spyOn(document, 'removeEventListener')
    const count = (spy: typeof added) => spy.mock.calls.filter(([type]) => type === EVENT).length

    const wrapper = mount(Sonner, { attachTo: document.body })
    for (const index of [1, 2, 3]) toast(`toast ${index}`)
    await flushPromises()
    await nextTick()
    expect(count(added)).toBeGreaterThanOrEqual(3)

    toast.dismiss()
    wrapper.unmount()
    await flushPromises()

    expect(count(removed)).toBe(count(added))
  })
})
