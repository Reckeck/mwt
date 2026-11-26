import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import StoreNodeSelect from './StoreNodeSelect.vue'

const node = (id: string, name: string, path: string[]) =>
  ({
    store: { id, name, kind: 'bank', color: '', archived: false, purpose: 'everyday', binding: 'none' },
    path,
    children: [],
    balances: [],
    subtree_base: '0',
  }) as unknown as StoreTreeNode

const NODES = [node('root', 'Bank', ['Bank']), node('child', 'Card', ['Bank', 'Card'])]

async function open(props: { modelValue: string; emptyLabel?: string }) {
  const wrapper = mount(StoreNodeSelect, { props: { nodes: NODES, ...props }, attachTo: document.body })
  await wrapper.find('button').trigger('click')
  await flushPromises()
  return wrapper
}

const rows = () => [...document.body.querySelectorAll<HTMLButtonElement>('[role="dialog"] button')]

describe('StoreNodeSelect', () => {
  beforeEach(() => setActivePinia(createPinia()))
  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('lists a sub-account the caller passes', async () => {
    await open({ modelValue: '' })
    const text = rows().map((row) => row.textContent)
    expect(text.some((line) => line?.includes('Card'))).toBe(true)
  })

  it('clears an optional pick through the empty row', async () => {
    const wrapper = await open({ modelValue: 'child', emptyLabel: 'Nothing' })
    const clear = rows().find((row) => row.textContent?.trim() === 'Nothing')
    expect(clear).toBeDefined()
    clear?.click()
    await flushPromises()
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([''])
  })
})
