import { flushPromises, mount } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'

import PlanNext from './PlanNext.vue'

const mounted: ReturnType<typeof mount>[] = []

function mountNext() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = mount(PlanNext, {
    global: {
      plugins: [[VueQueryPlugin, { queryClient: client }]],
      stubs: { RouterLink: { template: '<a><slot /></a>' }, Money: true },
    },
  })
  mounted.push(wrapper)
  return wrapper
}

afterEach(() => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
  vi.mocked(call).mockReset()
})

describe('PlanNext', () => {
  it('keeps the section and its tour anchor while pending, with a h-16 skeleton', () => {
    vi.mocked(call).mockImplementation(() => new Promise(() => undefined))
    const wrapper = mountNext()
    expect(wrapper.find('[data-tour="plan-next"]').exists()).toBe(true)
    expect(wrapper.find('.h-16').exists()).toBe(true)
  })

  it('shows one h-16 line for an empty list', async () => {
    vi.mocked(call).mockResolvedValue([])
    const wrapper = mountNext()
    await flushPromises()
    expect(wrapper.find('[data-tour="plan-next"]').exists()).toBe(true)
    const line = wrapper.find('p.h-16')
    expect(line.text()).toContain('ничего не запланировано')
    expect(wrapper.find('ul').exists()).toBe(false)
  })

  it('shows one h-16 line when the query is rejected', async () => {
    vi.mocked(call).mockRejectedValue(new Error('boom'))
    const wrapper = mountNext()
    await flushPromises()
    expect(wrapper.find('[data-tour="plan-next"]').exists()).toBe(true)
    expect(wrapper.find('p.h-16').text()).toContain('Не удалось')
  })

  it('lists the items when filled', async () => {
    vi.mocked(call).mockResolvedValue([
      { kind: 'reminder', source_id: 'r1', at: 1, date: '2026-11-20', title: 'Rent', amount: null, asset: null },
    ])
    const wrapper = mountNext()
    await flushPromises()
    expect(wrapper.find('[data-tour="plan-next"]').exists()).toBe(true)
    expect(wrapper.findAll('li')).toHaveLength(1)
    expect(wrapper.text()).toContain('Rent')
  })
})
