import { defineComponent } from 'vue'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { describe, expect, it } from 'vitest'

import { useTxFormState } from './use-tx-form-state'
import type { TransactionFormProps } from '../transaction-form.types'

const prefill = {
  payment: {
    store_id: 's1',
    asset: { symbol: 'RUB', name: 'RUB', coingecko_id: null, chain: null, contract: null },
    amount: '100',
    to: null,
    details: { tx_hash: '0xabc', network: null, source_store_id: null, unit_price: null },
    price: { price_usd: '0.0125', price_base: null, price_base_currency: null },
    fee: null,
    meta: { categories: [], comment: '', timestamp: '2026-09-01T10:00:00.123Z', contact_id: null, flow_id: 'flow1' },
  },
} as unknown as Transaction

describe('useTxFormState: Repeat', () => {
  it('opens without the price, the moment and the one-off ids', () => {
    const props = { storeId: 's1', prefill, transaction: null } as unknown as TransactionFormProps
    let state!: ReturnType<typeof useTxFormState>
    const host = defineComponent({
      setup() {
        state = useTxFormState(props)
        return {}
      },
      template: '<div />',
    })
    mount(host, { global: { plugins: [createPinia(), [VueQueryPlugin, { queryClient: new QueryClient() }]] } })
    const { form } = state
    expect(form.price).toBe('')
    expect(form.priceCurrency).toBe('')
    expect(form.timestamp).toBe('')
    expect(form.originalTimestamp).toBe('')
    expect(form.flowId).toBe('')
    expect(form.txHash).toBe('')
    expect(form.orderId).toBe('')
    expect(form.amount).toBe('100')
  })
})

describe('useTxFormState: Repeat of for_month', () => {
  it('opens without the month label and sends no for_month', () => {
    const withMonth = structuredClone(prefill) as unknown as { payment: { details: Record<string, unknown> } }
    withMonth.payment.details.for_month = '2026-09'
    const props = { storeId: 's1', prefill: withMonth, transaction: null } as unknown as TransactionFormProps
    let state!: ReturnType<typeof useTxFormState>
    const host = defineComponent({
      setup() {
        state = useTxFormState(props)
        return {}
      },
      template: '<div />',
    })
    mount(host, { global: { plugins: [createPinia(), [VueQueryPlugin, { queryClient: new QueryClient() }]] } })
    expect(state.form.forMonth).toBe('')
  })
})
