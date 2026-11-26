import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { builtinThemes } from '@/shared/lib/theme/registry'

import ThemePicker from './ThemePicker.vue'

function mountPicker(modes: 'light' | 'dark' | 'both', selectedId = 'default-light') {
  return mount(ThemePicker, { props: { modes, selectedId }, attachTo: document.body })
}

describe('ThemePicker', () => {
  it('lists every theme with five swatches when both modes are shown', () => {
    const wrapper = mountPicker('both')
    const radios = wrapper.findAll('[role="radio"]')
    expect(radios).toHaveLength(builtinThemes.length)
    expect(radios[0].findAll('[data-swatch]')).toHaveLength(5)
    expect(wrapper.findAll('h3')).toHaveLength(2)
  })

  it('lists only the themes of the shown mode', () => {
    const wrapper = mountPicker('dark', 'default-dark')
    const count = builtinThemes.filter((theme) => theme.mode === 'dark').length
    expect(wrapper.findAll('[role="radio"]')).toHaveLength(count)
    expect(wrapper.findAll('h3')).toHaveLength(1)
  })

  it('colours swatches from the theme data', () => {
    const wrapper = mountPicker('light')
    const first = wrapper.find('[data-swatch]').attributes('style') ?? ''
    expect(first).toContain('background-color')
  })

  it('marks only the selected theme', () => {
    const wrapper = mountPicker('both', 'dracula')
    const checked = wrapper.findAll('[aria-checked="true"]')
    expect(checked).toHaveLength(1)
    expect(checked[0].attributes('data-theme-id')).toBe('dracula')
  })

  it('emits select on click and on Enter', async () => {
    const wrapper = mountPicker('both')
    const radios = wrapper.findAll('[role="radio"]')
    await radios[1].trigger('click')
    await radios[2].trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('select')).toEqual([
      [radios[1].attributes('data-theme-id')],
      [radios[2].attributes('data-theme-id')],
    ])
  })

  it('moves the focus with the arrows and keeps one tab stop', async () => {
    const wrapper = mountPicker('both')
    const radios = wrapper.findAll('[role="radio"]')
    expect(radios.filter((radio) => radio.attributes('tabindex') === '0')).toHaveLength(1)
    await radios[0].trigger('keydown', { key: 'ArrowRight' })
    expect(document.activeElement).toBe(radios[1].element)
    await radios[1].trigger('keydown', { key: 'ArrowLeft' })
    expect(document.activeElement).toBe(radios[0].element)
  })
})
