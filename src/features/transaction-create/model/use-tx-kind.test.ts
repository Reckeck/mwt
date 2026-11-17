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

async function repeatUnsupported(links: { flowId?: string; propertyId?: string }) {
  const reactiveForm = reactive({
    ...emptyForm('s1', 'payment'),
    kind: 'trade',
    amount: '100',
    comment: 'copied',
    flowId: 'old-flow',
    propertyId: 'old-property',
  })
  const state = {
    props: { storeId: 's1', prefill: {}, transaction: null, ...links },
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
  return reactiveForm
}

describe('useTxKind: Repeat of an unsupported kind', () => {
  it('opens the wallet normal new form instead of switching a copy', async () => {
    const form = await repeatUnsupported({})
    expect(form.kind).toBe('payment')
    expect(form.amount).toBe('')
    expect(form.comment).toBe('')
  })

  it('keeps the flow and property the form was opened with', async () => {
    const form = await repeatUnsupported({ flowId: 'flow-1', propertyId: 'prop-1' })
    expect(form.flowId).toBe('flow-1')
    expect(form.propertyId).toBe('prop-1')
  })

  it('leaves them empty when the form was opened without them', async () => {
    const form = await repeatUnsupported({})
    expect(form.flowId).toBe('')
    expect(form.propertyId).toBe('')
  })
})
