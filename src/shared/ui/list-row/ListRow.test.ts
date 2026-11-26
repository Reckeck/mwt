import { mount } from '@vue/test-utils'
import { Wallet } from 'lucide-vue-next'
import { describe, expect, it } from 'vitest'

import ListRow from './ListRow.vue'

describe('ListRow', () => {
  it('shows icon, title, subtitle and value', () => {
    const wrapper = mount(ListRow, { props: { title: 'Tinkoff', subtitle: 'Card', icon: Wallet, value: '1 000 $' } })
    expect(wrapper.find('[data-list-row-icon]').exists()).toBe(true)
    expect(wrapper.find('[data-list-row-title]').text()).toBe('Tinkoff')
    expect(wrapper.find('[data-list-row-subtitle]').text()).toBe('Card')
    expect(wrapper.find('[data-list-row-value]').text()).toBe('1 000 $')
  })

  it('colours the value by tone and defaults to ink', () => {
    const base = { title: 'A', value: '1' }
    expect(mount(ListRow, { props: base }).find('[data-list-row-value]').classes()).toContain('text-foreground')
    expect(
      mount(ListRow, { props: { ...base, tone: 'positive' } })
        .find('[data-list-row-value]')
        .classes(),
    ).toContain('text-money-income')
    expect(
      mount(ListRow, { props: { ...base, tone: 'negative' } })
        .find('[data-list-row-value]')
        .classes(),
    ).toContain('text-money-expense')
  })

  it('leaves out the mark, subtitle and value when not given', () => {
    const wrapper = mount(ListRow, { props: { title: 'A' } })
    expect(wrapper.find('[data-list-row-icon]').exists()).toBe(false)
    expect(wrapper.find('[data-list-row-subtitle]').exists()).toBe(false)
    expect(wrapper.find('[data-list-row-value]').exists()).toBe(false)
  })
})
