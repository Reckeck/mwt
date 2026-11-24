import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ChartLegend from './ChartLegend.vue'

describe('ChartLegend', () => {
  it('lists every series with a token swatch', () => {
    const wrapper = mount(ChartLegend, {
      props: {
        items: [
          { label: 'Actual', tone: 'actual' },
          { label: 'Plan', tone: 'reference', dashed: true },
        ],
      },
    })
    expect(wrapper.findAll('li')).toHaveLength(2)
    expect(wrapper.text()).toContain('Plan')
    expect(wrapper.findAll('li')[1]?.find('span').classes()).toContain('border-dashed')
  })

  it('renders nothing for a single series', () => {
    const wrapper = mount(ChartLegend, { props: { items: [{ label: 'Actual', tone: 'actual' }] } })
    expect(wrapper.find('ul').exists()).toBe(false)
  })
})
