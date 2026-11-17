import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { defineComponent, h, nextTick, ref, type Ref } from 'vue'

import { holdPlan, providePlanReady } from './use-plan-ready'

const Block = defineComponent({
  props: { pending: { type: Object as () => Ref<boolean>, required: true } },
  setup(props) {
    holdPlan(() => props.pending.value)
    return () => h('section')
  },
})

function mountPlan(...pending: Ref<boolean>[]) {
  const Plan = defineComponent({
    setup() {
      const ready = providePlanReady()
      return () =>
        h(
          'div',
          { 'data-ready': String(ready.value) },
          pending.map((one) => h(Block, { pending: one })),
        )
    },
  })
  return mount(Plan)
}

const readiness = (wrapper: ReturnType<typeof mountPlan>) => wrapper.find('div').attributes('data-ready')

describe('providePlanReady', () => {
  it('waits for every block to have its first data', async () => {
    const limits = ref(true)
    const next = ref(true)
    const wrapper = mountPlan(limits, next)
    await nextTick()
    expect(readiness(wrapper)).toBe('false')
    limits.value = false
    await nextTick()
    expect(readiness(wrapper)).toBe('false')
    next.value = false
    await nextTick()
    expect(readiness(wrapper)).toBe('true')
  })

  it('stays ready when a block refetches', async () => {
    const limits = ref(false)
    const wrapper = mountPlan(limits)
    await nextTick()
    expect(readiness(wrapper)).toBe('true')
    limits.value = true
    await nextTick()
    expect(readiness(wrapper)).toBe('true')
  })

  it('lets a block hold nothing outside the plan', () => {
    expect(() => mount(Block, { props: { pending: ref(true) } })).not.toThrow()
  })
})
