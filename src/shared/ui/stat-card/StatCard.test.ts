import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import StatCard from './StatCard.vue'

describe('StatCard', () => {
  it('draws the delta line with a sign and colour by direction', () => {
    const wrapper = mount(StatCard, { props: { label: 'Total', value: '10', delta: 4.2, deltaText: '4.2%' } })
    expect(wrapper.text()).toContain('+4.2%')
    expect(wrapper.find('.text-success').exists()).toBe(true)
  })

  it('flips the colour for an inverse delta', () => {
    const wrapper = mount(StatCard, {
      props: { label: 'Spent', value: '10', delta: 4.2, deltaText: '4.2%', deltaInverse: true },
    })
    expect(wrapper.find('.text-destructive').exists()).toBe(true)
  })

  it('has no delta line without a delta', () => {
    expect(mount(StatCard, { props: { label: 'Total', value: '10' } }).text()).not.toContain('%')
  })
})
