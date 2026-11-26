import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import RangeSwitch from './RangeSwitch.vue'

describe('RangeSwitch', () => {
  it('shows 1M, 3M, 1Y and All with the open range lit', () => {
    const wrapper = mount(RangeSwitch, { props: { modelValue: 'last3' } })
    const items = wrapper.findAll('[data-range]')
    expect(items).toHaveLength(4)
    expect(wrapper.get('[data-range="last3"]').attributes('data-state')).toBe('on')
    expect(wrapper.get('[data-range="all"]').attributes('data-state')).toBe('off')
  })

  it('emits the picked range and ignores a click that would clear it', async () => {
    const wrapper = mount(RangeSwitch, { props: { modelValue: 'last3' } })
    await wrapper.get('[data-range="all"]').trigger('click')
    expect(wrapper.emitted('update:modelValue')).toEqual([['all']])
    await wrapper.get('[data-range="last3"]').trigger('click')
    expect(wrapper.emitted('update:modelValue')).toHaveLength(1)
  })
})
