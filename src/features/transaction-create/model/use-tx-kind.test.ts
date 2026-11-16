import { defineComponent, reactive } from 'vue'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { createI18n } from 'vue-i18n'
import { describe, expect, it, vi } from 'vitest'

import { emptyForm } from '../transaction-form.model'
import { useTxKind } from './use-tx-kind'
import type { TxFormState } from './use-tx-form-state'

vi.mock('@/shared/api/ipc', () => ({
  call: vi.fn(async () => ({ verbs: ['payment', 'topup'], special: [], default: 'payment' })),
}))

describe('useTxKind: Repeat of an unsupported kind', () => {
  it('opens the wallet normal new form instead of switching a copy', async () => {
    const reactiveForm = reactive({ ...emptyForm('s1', 'payment'), kind: 'trade', amount: '100', comment: 'copied' })
    const state = {
      props: { storeId: 's1', prefill: {}, transaction: null },
      form: reactiveForm,
      editing: { value: false },
      storeKind: { value: 'cash' },
      isProperty: { value: false },
    } as unknown as TxFormState
    const host = defineComponent({
      setup() {
        useTxKind(state)
        return {}
      },
      template: '<div />',
    })
    mount(host, {
      global: {
        plugins: [
          createPinia(),
          createI18n({ legacy: false, locale: 'en', messages: { en: {} } }),
          [VueQueryPlugin, { queryClient: new QueryClient() }],
        ],
      },
    })
    await flushPromises()
    expect(reactiveForm.kind).toBe('payment')
    expect(reactiveForm.amount).toBe('')
    expect(reactiveForm.comment).toBe('')
  })
})
