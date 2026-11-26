import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { Tabs } from '.'

describe('Tabs', () => {
  it('exposes the open tab with aria-selected', () => {
    const wrapper = mount(Tabs, {
      props: {
        modelValue: 'b',
        tabs: [
          { value: 'a', label: 'A' },
          { value: 'b', label: 'B' },
        ],
      },
    })
    const [first, second] = wrapper.findAll('[role="tab"]')
    expect(first.attributes('aria-selected')).toBe('false')
    expect(second.attributes('aria-selected')).toBe('true')
  })

  it('wears a pill: the strip and the tabs are fully rounded', () => {
    const wrapper = mount(Tabs, { props: { modelValue: 'a', tabs: [{ value: 'a', label: 'A' }] } })
    expect(wrapper.get('[role="tablist"]').classes()).toContain('rounded-full')
    expect(wrapper.get('[role="tab"]').classes()).toContain('rounded-full')
  })
})
