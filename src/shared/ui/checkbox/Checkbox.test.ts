import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

import { Checkbox } from '@/shared/ui/checkbox'

// Reka renders the root through a fragment, so the button is found by role.
function box(props: Record<string, unknown>) {
  const wrapper = mount(Checkbox, { props })
  return { wrapper, button: wrapper.get('[role=checkbox]') }
}

describe('Checkbox', () => {
  it('reflects the model value in its state', () => {
    expect(box({ modelValue: true }).button.attributes('data-state')).toBe('checked')
    expect(box({ modelValue: false }).button.attributes('data-state')).toBe('unchecked')
  })

  it('emits the new value on click', async () => {
    const { wrapper, button } = box({ modelValue: false })
    await button.trigger('click')
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([true])
  })

  it('does not emit when disabled', async () => {
    const { wrapper, button } = box({ modelValue: false, disabled: true })
    await button.trigger('click')
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
  })
})
