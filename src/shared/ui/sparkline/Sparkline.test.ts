import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import Sparkline from './Sparkline.vue'

describe('Sparkline', () => {
  it('is an image with a label and a chart token colour', () => {
    const svg = mount(Sparkline, { props: { values: [1, 3, 2], label: 'Net worth trend', tone: 'income' } }).find('svg')
    expect(svg.attributes('role')).toBe('img')
    expect(svg.attributes('aria-label')).toBe('Net worth trend')
    expect(svg.classes()).toContain('text-chart-income')
  })

  it('is hidden from assistive tech without a label', () => {
    const svg = mount(Sparkline, { props: { values: [1, 3, 2] } }).find('svg')
    expect(svg.attributes('aria-hidden')).toBe('true')
    expect(svg.classes()).toContain('text-chart-actual')
  })
})
