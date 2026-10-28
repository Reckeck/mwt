import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { nextTick } from 'vue'

import { GROUP_SEPARATOR } from '@/shared/lib/amount-input'

import AmountInput from './AmountInput.vue'
import type { AmountExpression } from './amount-input.types'

function setup(props: Record<string, unknown> = {}, value = '') {
  const wrapper = mount(AmountInput, {
    props: {
      modelValue: value,
      'onUpdate:modelValue': (next: string | number) => wrapper.setProps({ modelValue: next }),
      ...props,
    },
    attachTo: document.body,
  })
  const input = wrapper.find('input')
  async function type(text: string) {
    await input.setValue(text)
    await nextTick()
  }
  const hint = () => wrapper.find('[data-amount-hint]')
  const model = () => wrapper.props('modelValue') as string
  const expressions = () => (wrapper.emitted('expression') ?? []).map((args) => args[0] as AmountExpression)
  return { wrapper, input, type, hint, model, expressions }
}

describe('AmountInput', () => {
  it('keeps the plain number path: grouping, canonical model, no hint', async () => {
    const { input, type, hint, model, expressions } = setup()
    await type('1234,5')
    expect((input.element as HTMLInputElement).value).toMatch(/^1\u202f234[.,]5$/)
    expect(model()).toBe('1234.5')
    expect(hint().exists()).toBe(false)
    expect(expressions()).toEqual([])
  })

  it('keeps a leading minus a sign when negatives are allowed', async () => {
    const { type, hint, model } = setup({ allowNegative: true })
    await type('-15')
    expect(model()).toBe('-15')
    expect(hint().exists()).toBe(false)
  })

  it('shows the result of an expression and leaves the model alone until it is finished', async () => {
    const { input, type, hint, model } = setup({}, '5')
    await type('1200+350*2')
    expect(hint().text()).toBe(`= 1${GROUP_SEPARATOR}900`)
    expect((input.element as HTMLInputElement).value).toBe('1200+350*2')
    expect(model()).toBe('5')
  })

  it('Enter puts the result in the field and reports the expression', async () => {
    const { input, type, hint, model, expressions } = setup()
    await type('1200+350*2')
    await input.trigger('keydown', { key: 'Enter' })
    expect((input.element as HTMLInputElement).value).toBe(`1${GROUP_SEPARATOR}900`)
    expect(model()).toBe('1900')
    expect(hint().exists()).toBe(false)
    expect(expressions()).toEqual([{ expr: '1200+350*2', result: '1900' }])
  })

  it('leaving the field does the same', async () => {
    const { input, type, model, expressions } = setup()
    await type('100/8')
    await input.trigger('blur')
    expect(model()).toBe('12.5')
    expect(expressions()).toEqual([{ expr: '100/8', result: '12.5' }])
  })

  it('Esc puts back what the field held at focus', async () => {
    const { input, type, hint, model, expressions } = setup({}, '700')
    await input.trigger('focus')
    await type('1+2')
    await input.trigger('keydown', { key: 'Escape' })
    expect((input.element as HTMLInputElement).value).toBe('700')
    expect(model()).toBe('700')
    expect(hint().exists()).toBe(false)
    expect(expressions()).toEqual([])
  })

  it('does not let Enter or Esc out of the field while an expression is open', async () => {
    const { input, type } = setup()
    await type('1+2')
    const enter = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })
    input.element.dispatchEvent(enter)
    expect(enter.defaultPrevented).toBe(true)
    // A plain number's Enter still submits the form.
    const plain = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })
    input.element.dispatchEvent(plain)
    expect(plain.defaultPrevented).toBe(false)
  })

  it('shows division by zero and keeps the old amount', async () => {
    const { input, type, hint, model, expressions } = setup({}, '300')
    await input.trigger('focus')
    await type('500/0')
    expect(hint().text()).toBe('Деление на ноль')
    await input.trigger('keydown', { key: 'Enter' })
    expect(model()).toBe('300')
    expect(expressions()).toEqual([])
    await input.trigger('blur')
    expect((input.element as HTMLInputElement).value).toBe('300')
    expect(hint().exists()).toBe(false)
  })

  it('says «could not calculate» only after the user tries to finish a broken expression', async () => {
    const { input, type, hint, model } = setup({}, '300')
    await type('1200+')
    expect(hint().exists()).toBe(false)
    await input.trigger('keydown', { key: 'Enter' })
    expect(hint().text()).toBe('Не получилось посчитать')
    expect(model()).toBe('300')
  })

  it('refuses a negative result in a positive-only field', async () => {
    const { input, type, hint, model } = setup({}, '1')
    await type('5-9')
    expect(hint().text()).toBe('Сумма не может быть отрицательной')
    await input.trigger('keydown', { key: 'Enter' })
    expect(model()).toBe('1')
  })

  it('rounds a result to the field decimals', async () => {
    const { input, type, model } = setup({ decimals: 2 })
    await type('2/3')
    await input.trigger('keydown', { key: 'Enter' })
    expect(model()).toBe('0.67')
  })

  it('accepts a negative result when negatives are allowed', async () => {
    const { input, type, model } = setup({ allowNegative: true })
    await type('5-9')
    await input.trigger('keydown', { key: 'Enter' })
    expect(model()).toBe('-4')
  })

  it('reformats an outside change as before', async () => {
    const { wrapper, input } = setup()
    await wrapper.setProps({ modelValue: '12345.67' })
    expect((input.element as HTMLInputElement).value).toMatch(/^12\u202f345[.,]67$/)
  })
})
