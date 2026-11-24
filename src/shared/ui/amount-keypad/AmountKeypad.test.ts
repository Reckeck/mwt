import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'

const phone = ref(true)
vi.mock('@/shared/lib/phone', () => ({ useIsPhone: () => phone }))

import AmountKeypad from './AmountKeypad.vue'

describe('AmountKeypad', () => {
  it('emits value events only', async () => {
    const wrapper = mount(AmountKeypad, { props: { decimalMark: ',' } })
    for (const id of ['7', 'decimal', 'backspace', '*']) {
      await wrapper.get(`[data-pad-key="${id}"]`).trigger('click')
    }
    expect(wrapper.emitted()).toMatchObject({
      digit: [['7']],
      decimal: [[]],
      backspace: [[]],
      operator: [['*']],
    })
    expect(wrapper.text()).toContain(',')
  })

  it('has an operator column and keys of 40 px or more', () => {
    const wrapper = mount(AmountKeypad, { props: { decimalMark: '.' } })
    for (const id of ['+', '-', '*', '/']) expect(wrapper.find(`[data-pad-key="${id}"]`).exists()).toBe(true)
    for (const key of wrapper.findAll('[data-pad-key]')) {
      expect(key.classes()).toContain('h-14')
      expect(key.classes()).toContain('min-w-10')
    }
  })

  it('renders nothing on desktop', () => {
    phone.value = false
    const wrapper = mount(AmountKeypad, { props: { decimalMark: '.' } })
    expect(wrapper.find('[data-amount-pad]').exists()).toBe(false)
    phone.value = true
  })
})
