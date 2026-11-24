import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import Delta from './Delta.vue'

describe('Delta', () => {
  it('shows a plus sign, text and the success colour for a rise', () => {
    const wrapper = mount(Delta, { props: { value: 4.2, text: '4.2%' } })
    expect(wrapper.text()).toBe('+4.2%')
    expect(wrapper.classes()).toContain('text-success')
  })

  it('shows a minus sign and the destructive colour for a fall', () => {
    const wrapper = mount(Delta, { props: { value: '-1.5' } })
    expect(wrapper.text()).toBe('−1.5')
    expect(wrapper.classes()).toContain('text-destructive')
  })

  it('has no sign and no colour for zero', () => {
    const wrapper = mount(Delta, { props: { value: 0, text: '0%' } })
    expect(wrapper.text()).toBe('0%')
    expect(wrapper.classes()).not.toContain('text-success')
  })

  it('flips the colour, not the sign, when a rise is bad', () => {
    const wrapper = mount(Delta, { props: { value: 3, inverse: true } })
    expect(wrapper.text()).toBe('+3')
    expect(wrapper.classes()).toContain('text-destructive')
  })
})
