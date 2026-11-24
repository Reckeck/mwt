import { mount } from '@vue/test-utils'
import { Wallet } from 'lucide-vue-next'
import { describe, expect, it } from 'vitest'

import IconBadge from './IconBadge.vue'

describe('IconBadge', () => {
  it('draws the glyph on the sunken circle by default', () => {
    const wrapper = mount(IconBadge, { props: { icon: Wallet } })
    expect(wrapper.classes()).toContain('bg-muted')
    expect(wrapper.classes()).toContain('size-10')
    expect(wrapper.find('svg').attributes('stroke-width')).toBe('1.75')
  })

  it('uses the soft positive tone for income', () => {
    const wrapper = mount(IconBadge, { props: { icon: Wallet, tone: 'positive', size: 'sm' } })
    expect(wrapper.classes()).toContain('bg-success-soft')
    expect(wrapper.classes()).toContain('size-8')
    expect(wrapper.find('svg').classes()).toContain('size-4')
  })
})
