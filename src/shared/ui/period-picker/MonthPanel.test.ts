import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { MIN_DATE_YEAR } from '@/shared/lib/limits'

import MonthPanel from './MonthPanel.vue'

describe('MonthPanel bounds', () => {
  it('disables the previous year and every month before the first allowed year', () => {
    const wrapper = mount(MonthPanel, {
      props: { modelValue: { from: `${MIN_DATE_YEAR}-02`, to: `${MIN_DATE_YEAR}-02` } },
    })
    const buttons = wrapper.findAll('button')
    expect(buttons[0].attributes('disabled')).toBeDefined()
    expect(buttons[1].attributes('disabled')).toBeUndefined()
  })

  it('keeps the previous year open one year after the edge', () => {
    const wrapper = mount(MonthPanel, {
      props: { modelValue: { from: `${MIN_DATE_YEAR + 1}-02`, to: `${MIN_DATE_YEAR + 1}-02` } },
    })
    expect(wrapper.findAll('button')[0].attributes('disabled')).toBeUndefined()
  })

  it('disables the next year at the current one', () => {
    const now = new Date()
    const key = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`
    const wrapper = mount(MonthPanel, { props: { modelValue: { from: key, to: key } } })
    expect(wrapper.findAll('button')[1].attributes('disabled')).toBeDefined()
  })
})
