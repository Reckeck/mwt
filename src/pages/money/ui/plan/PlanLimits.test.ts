import { mount } from '@vue/test-utils'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'

import { call } from '@/shared/api/ipc'

import PlanLimits from './PlanLimits.vue'

vi.mock('@/shared/api/queries', () => ({
  useCategories: () => ({ data: ref([]) }),
  useStoreTree: () => ({ data: ref([]) }),
}))
vi.mock('@/widgets/analytics-reports', () => ({ useReport: () => ({ data: ref(undefined) }) }))

const mounted: ReturnType<typeof mount>[] = []

function mountLimits() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: '/', component: { template: '<div />' } }],
  })
  const wrapper = mount(PlanLimits, {
    props: { filter: {} as ReportFilter },
    global: {
      plugins: [[VueQueryPlugin, { queryClient: client }], router],
      stubs: { BudgetDialog: true, BudgetDeleteDialog: true, Money: true },
    },
  })
  mounted.push(wrapper)
  return wrapper
}

afterEach(() => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
  vi.mocked(call).mockReset()
})

describe('PlanLimits', () => {
  it('renders a skeleton in place of the rows while the statuses are pending', () => {
    vi.mocked(call).mockImplementation(() => new Promise(() => undefined))
    const wrapper = mountLimits()
    expect(wrapper.find('[data-tour="plan-limits"]').exists()).toBe(true)
    expect(wrapper.find('[data-slot="skeleton"]').exists()).toBe(true)
    expect(wrapper.find('ul').exists()).toBe(false)
  })
})
