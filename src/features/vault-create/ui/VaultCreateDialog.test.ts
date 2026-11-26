import { defineComponent } from 'vue'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

import VaultCreateDialog from './VaultCreateDialog.vue'

const replace = vi.hoisted(() => vi.fn(async () => undefined))
vi.mock('vue-router', () => ({ useRouter: () => ({ replace }) }))

/** The real picker needs the query client; the dialog only passes the code through. */
const CurrencyStub = defineComponent({
  props: { modelValue: { type: String, default: '' } },
  emits: ['update:modelValue'],
  template:
    '<button id="currency-clear" type="button" @click="$emit(\'update:modelValue\', \'\')">{{ modelValue }}</button>',
})

const mounted: ReturnType<typeof mount>[] = []
const created = () => vi.mocked(call).mock.calls.filter(([command]) => command === 'create_vault')

beforeEach(() => {
  setActivePinia(createPinia())
  vi.mocked(call).mockImplementation(async (command: string) => {
    if (command === 'vault_status') return { exists: true, storage: 'file', path: '' }
    if (command === 'create_vault' || command === 'update_app_settings') return undefined
    throw new Error(`unmocked ipc call: ${command}`)
  })
})

afterEach(() => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
  document.body.innerHTML = ''
  vi.clearAllMocks()
})

function field(id: string): HTMLInputElement {
  const element = document.body.querySelector<HTMLInputElement>(`#${id}`)
  if (!element) throw new Error(`no field #${id}`)
  return element
}

async function type(id: string, value: string) {
  const element = field(id)
  element.value = value
  element.dispatchEvent(new Event('input', { bubbles: true }))
  await flushPromises()
}

async function submit() {
  document.body.querySelector('form')?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }))
  await flushPromises()
}

async function open(asked = false) {
  setActivePinia(createPinia())
  if (asked) useSettingsStore().$patch({ app: { base_currency_chosen: false } as never })
  const wrapper = mount(VaultCreateDialog, {
    attachTo: document.body,
    props: { open: false },
    global: { stubs: { BaseCurrencyField: CurrencyStub } },
  })
  mounted.push(wrapper)
  await wrapper.setProps({ open: true })
  await flushPromises()
  await type('create-path', '/tmp/new.wallet')
  return wrapper
}

describe('VaultCreateDialog validation', () => {
  it('shows the field error and does not create with a short password', async () => {
    await open()
    await type('create-password-new', '123')
    await type('create-password-repeat', '123')
    await submit()
    expect(document.body.textContent).toContain('Не короче 6 символов')
    expect(created()).toHaveLength(0)
  })

  it('shows auth.passwordMismatch when the repeat differs', async () => {
    await open()
    await type('create-password-new', '123456')
    await type('create-password-repeat', '654321')
    await submit()
    expect(document.body.textContent).toContain('Пароли не совпадают')
    expect(created()).toHaveLength(0)
  })

  it('submits a plain vault with no password', async () => {
    await open()
    field('vault-protection-plain').click()
    await flushPromises()
    await submit()
    expect(created()).toHaveLength(1)
    expect(JSON.stringify(created()[0])).not.toContain('123')
  })

  it('goes to the currency step when asked, and an empty currency blocks Create', async () => {
    await open(true)
    await type('create-password-new', '123456')
    await type('create-password-repeat', '123456')
    await submit()
    expect(document.body.querySelector('#create-password-new')).toBeNull()
    expect(created()).toHaveLength(0)
    expect(document.body.textContent).toContain('Базовая валюта')
    field('currency-clear').click()
    await flushPromises()
    await submit()
    expect(created()).toHaveLength(0)
    expect(document.body.textContent).toContain('Шаг 2 из 2')
  })
})
