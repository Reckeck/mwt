import { VueQueryPlugin } from '@tanstack/vue-query'
import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it } from 'vitest'

import Money from './Money.vue'

function render(flow: 'expense' | 'income' | 'move', amount: string) {
  const wrapper = mount(Money, { props: { amount, currency: 'usd', flow }, global: { plugins: [VueQueryPlugin] } })
  const sum = wrapper.find('.money')
  return { text: sum.text(), classes: sum.classes() }
}

describe('Money flow colours (D7)', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('expense is ink with a minus', () => {
    const { text, classes } = render('expense', '12.5')
    expect(text).toMatch(/^[-−]/)
    expect(classes).toContain('text-foreground')
  })

  it('income is positive with a plus', () => {
    const { text, classes } = render('income', '40')
    expect(text).toMatch(/^\+/)
    expect(classes).toContain('text-success')
  })

  it('transfer or exchange is muted without a sign', () => {
    const { text, classes } = render('move', '-7')
    expect(text).not.toMatch(/^[-−+]/)
    expect(classes).toContain('text-muted-foreground')
  })
})
