import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import ChartTooltip from './ChartTooltip.vue'

describe('ChartTooltip', () => {
  it('shows the title and a labelled figure per series', () => {
    const wrapper = mount(ChartTooltip, {
      props: {
        title: '12 Nov',
        rows: [
          { label: 'Actual', value: '1 200' },
          { label: 'Gap', value: '-50', tone: 'negative' },
        ],
      },
    })
    expect(wrapper.text()).toContain('12 Nov')
    expect(wrapper.findAll('dt')).toHaveLength(2)
    expect(wrapper.findAll('dd')[1]?.classes()).toContain('text-destructive')
  })
})
