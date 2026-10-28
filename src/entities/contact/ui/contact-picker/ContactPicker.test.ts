import { flushPromises, mount } from '@vue/test-utils'
import { VueQueryPlugin, QueryClient } from '@tanstack/vue-query'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'

import { call } from '@/shared/api/ipc'

import { CONTACT_CREATOR_KEY as contactCreatorKey } from '@/shared/lib/contact-create'

import ContactPicker from './ContactPicker.vue'

const anna = { id: 'c-anna', name: 'Анна', kind: 'person', group: '', note: '', archived: false, accounts: [] }
const petya = { ...anna, id: 'c-petya', name: 'Петя' }

function backend() {
  vi.mocked(call).mockImplementation(async (cmd: string) => {
    if (cmd === 'list_contacts') return [anna]
    throw new Error(`unmocked ipc call: ${cmd}`)
  })
}

/** What the shell provides: the form is closed with a saved contact, or unsaved (`null`). */
const creator = vi.fn(async (_name: string): Promise<Contact | null> => petya as unknown as Contact)

const mounted: ReturnType<typeof mount>[] = []

/** A parent holding the model, like a form does. */
function mountPicker(props: Record<string, unknown> = {}, provided = true) {
  const picked = ref('')
  const Host = defineComponent({
    setup: () => () =>
      h(ContactPicker, {
        modelValue: picked.value,
        'onUpdate:modelValue': (id: string) => (picked.value = id),
        ...props,
      }),
  })
  const wrapper = mount(Host, {
    attachTo: document.body,
    global: {
      provide: provided ? { [contactCreatorKey as symbol]: creator } : {},
      plugins: [[VueQueryPlugin, { queryClient: new QueryClient({ defaultOptions: { queries: { retry: false } } }) }]],
    },
  })
  mounted.push(wrapper)
  return { wrapper, picked }
}

const input = () => document.body.querySelector('input') as HTMLInputElement
const options = () => [...document.body.querySelectorAll('[role=option]')].map((node) => node.textContent?.trim())

async function type(text: string) {
  input().focus()
  input().click()
  input().value = text
  input().dispatchEvent(new Event('input', { bubbles: true }))
  await flushPromises()
}

describe('ContactPicker', () => {
  beforeEach(() => {
    backend()
    creator.mockClear()
  })
  afterEach(() => {
    for (const wrapper of mounted.splice(0)) wrapper.unmount()
  })

  it('finds a contact by what is typed', async () => {
    mountPicker()
    await flushPromises()
    await type('анна')
    expect(options()).toEqual(['Анна'])
  })

  it('keeps «Создать» last when the name only partly matches', async () => {
    mountPicker()
    await flushPromises()
    await type('ан')
    expect(options()).toEqual(['Анна', 'Создать «ан»'])
  })

  it('offers «Создать «Петя»» when nobody matches', async () => {
    mountPicker()
    await flushPromises()
    await type('Петя')
    expect(options()).toEqual(['Создать «Петя»'])
  })

  it('offers nothing to create in a pick-only field', async () => {
    mountPicker({ noCreate: true })
    await flushPromises()
    await type('Петя')
    expect(options()).toEqual([])
  })

  it('hands the typed name to the shell form and picks the saved contact', async () => {
    const { picked } = mountPicker()
    await flushPromises()
    await type('Петя')
    ;(document.body.querySelector('[role=option]') as HTMLElement).click()
    await flushPromises()

    expect(creator).toHaveBeenCalledWith('Петя')
    expect(picked.value).toBe('c-petya')
  })

  it('keeps the old choice when the form is closed unsaved', async () => {
    creator.mockResolvedValueOnce(null)
    const { picked } = mountPicker()
    picked.value = 'c-anna'
    await flushPromises()
    await type('Петя')
    ;(document.body.querySelector('[role=option]') as HTMLElement).click()
    await flushPromises()

    expect(picked.value).toBe('c-anna')
  })

  it('only picks where no shell provides a form', async () => {
    mountPicker({}, false)
    await flushPromises()
    await type('Петя')
    expect(options()).toEqual([])
  })
})
