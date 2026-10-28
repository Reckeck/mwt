import { mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'
import { nextTick } from 'vue'

import AmountKeypad from './AmountKeypad.vue'

function key(name: string) {
  const button = document.querySelector<HTMLButtonElement>(`[data-calc-key="${name}"]`)
  if (!button) throw new Error(`no key ${name}`)
  return button
}
const text = (selector: string) => document.querySelector(selector)?.textContent?.trim()

async function open(initial = '') {
  const wrapper = mount(AmountKeypad, {
    props: { open: true, text: initial, decimalMark: ',', decimals: 2 },
    attachTo: document.body,
  })
  await nextTick()
  await nextTick()
  return wrapper
}

describe('AmountKeypad', () => {
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('builds an expression and hands it back on «Готово»', async () => {
    const wrapper = await open()
    for (const name of ['1', '2', '0', '0', '+', '3', '5', '0', '×', '2']) {
      key(name).click()
      await nextTick()
    }
    expect(text('[data-keypad-expr]')).toBe('1200+350×2')
    expect(text('[data-keypad-result]')).toMatch(/^= 1\s?900$/)
    document.querySelector<HTMLButtonElement>('[data-calc-done]')?.click()
    expect(wrapper.emitted('done')?.[0]).toEqual(['1200+350×2'])
    expect(wrapper.emitted('update:open')?.[0]).toEqual([false])
  })

  it('starts from the field text and erases', async () => {
    await open('1 900')
    expect(text('[data-keypad-expr]')).toBe('1900')
    key('back').click()
    await nextTick()
    expect(text('[data-keypad-expr]')).toBe('190')
    key('C').click()
    await nextTick()
    expect(text('[data-keypad-expr]')).toBe('0')
  })

  it('says «division by zero» and does not finish a broken expression', async () => {
    const wrapper = await open()
    for (const name of ['5', '÷', '0']) {
      key(name).click()
      await nextTick()
    }
    expect(text('[data-keypad-result]')).toBe('Деление на ноль')
    document.querySelector<HTMLButtonElement>('[data-calc-done]')?.click()
    expect(wrapper.emitted('done')).toBeUndefined()
  })

  it('«=» replaces the expression with its result', async () => {
    await open()
    for (const name of ['2', '+', '2']) {
      key(name).click()
      await nextTick()
    }
    key('=').click()
    await nextTick()
    expect(text('[data-keypad-expr]')).toBe('4')
  })
})
