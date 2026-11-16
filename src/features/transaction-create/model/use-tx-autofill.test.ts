import { defineComponent, nextTick, reactive, ref } from 'vue'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { describe, expect, it } from 'vitest'

import { emptyForm } from '../lib/form/form-state'
import { useTxAutofill } from './use-tx-autofill'
import type { TxFormState } from './use-tx-form-state'

function setup(options: { editing: boolean; prefill?: boolean }) {
  const form = reactive({ ...emptyForm('A', 'transfer'), to: { store: 'A' } })
  const state = {
    props: { storeId: 'A', prefill: options.prefill ? {} : undefined },
    form,
    editing: ref(options.editing),
    isProperty: ref(false),
    storeKind: ref('bank'),
    overview: { data: ref(undefined), isSuccess: ref(false) },
    contextStore: ref(undefined),
    properties: ref([]),
  } as unknown as TxFormState
  const host = defineComponent({ setup: () => useTxAutofill(state), template: '<div />' })
  mount(host, { global: { plugins: [createPinia(), [VueQueryPlugin, { queryClient: new QueryClient() }]] } })
  return form
}

describe('useTxAutofill: transfer destination', () => {
  it('keeps "to" when editing a transfer', async () => {
    const form = setup({ editing: true })
    await nextTick()
    expect(form.to).toEqual({ store: 'A' })
  })

  it('keeps "to" on a prefilled (repeat) form', async () => {
    const form = setup({ editing: false, prefill: true })
    await nextTick()
    expect(form.to).toEqual({ store: 'A' })
  })

  it('clears "to" on a new transfer form', async () => {
    const form = setup({ editing: false })
    await nextTick()
    expect(form.to).toBeNull()
  })
})
