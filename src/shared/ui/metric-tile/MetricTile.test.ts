import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import MetricTile from './MetricTile.vue'

describe('MetricTile', () => {
  it('writes a fall with exactly one minus sign', () => {
    const wrapper = mount(MetricTile, { props: { label: 'Income', deltaPct: -5, size: 's' } })
    const text = wrapper.text()
    expect(text.match(/−/g)).toHaveLength(1)
    expect(text).not.toContain('-')
    expect(text).toMatch(/5[.,]0/)
  })

  it('writes a rise with a plus sign only', () => {
    const wrapper = mount(MetricTile, { props: { label: 'Income', deltaPct: 5, size: 's' } })
    expect(wrapper.text()).toContain('+')
    expect(wrapper.text()).not.toMatch(/[−-]/)
  })
})
