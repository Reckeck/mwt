// check-ui-ignore raw-color: test fixtures, arbitrary colour values
import { nextTick } from 'vue'
import { afterEach, describe, expect, it } from 'vitest'

import { chartColors } from './chart-theme'

const root = document.documentElement

afterEach(() => {
  root.removeAttribute('style')
  root.removeAttribute('data-theme')
})

describe('chartColors', () => {
  it('reads the --chart-* tokens at runtime', () => {
    root.style.setProperty('--chart-actual', '#111111')
    root.style.setProperty('--chart-grid', '#222222')
    root.style.setProperty('--chart-heat-4', '#333333')
    const colors = chartColors()
    expect(colors.actual).toBe('#111111')
    expect(colors.grid).toBe('#222222')
    expect(colors.heat[4]).toBe('#333333')
  })

  it('re-reads the tokens after the theme attribute changes', async () => {
    root.style.setProperty('--chart-actual', '#111111')
    chartColors()
    root.style.setProperty('--chart-actual', '#999999')
    root.setAttribute('data-theme', 'dark')
    await nextTick()
    await Promise.resolve()
    expect(chartColors().actual).toBe('#999999')
  })
})
