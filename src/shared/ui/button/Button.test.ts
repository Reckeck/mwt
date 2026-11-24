import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { Button, buttonVariants } from '.'

describe('Button', () => {
  it('mixes hover and pressed states with color-mix', () => {
    for (const variant of ['default', 'secondary', 'ghost', 'destructive', 'chip'] as const) {
      const cls = buttonVariants({ variant })
      expect(cls).toMatch(/hover:bg-\[color-mix\(/)
      expect(cls).toMatch(/active:bg-\[color-mix\(/)
    }
  })

  it('shows a focus ring and a 40 px touch target on text sizes', () => {
    const cls = buttonVariants({ variant: 'default', size: 'default' })
    expect(cls).toContain('focus-visible:ring-2')
    expect(cls).toContain('touch-min')
    expect(buttonVariants({ variant: 'chip', size: 'chip' })).toContain('touch-min')
  })

  it('chip is a pill that turns ink when pressed', () => {
    const cls = buttonVariants({ variant: 'chip', size: 'chip' })
    expect(cls).toContain('rounded-full')
    expect(cls).toContain('aria-pressed:bg-primary')
  })

  it('keeps its label in place under the spinner while loading', () => {
    const wrapper = mount(Button, { props: { loading: true }, slots: { default: 'Save' } })
    const button = wrapper.get('button')
    expect(button.attributes('disabled')).toBeDefined()
    expect(button.attributes('aria-busy')).toBe('true')
    expect(wrapper.get('span.contents').classes()).toContain('invisible')
    expect(wrapper.text()).toContain('Save')
  })
})
