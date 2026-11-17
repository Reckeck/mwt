import { defineComponent, nextTick, reactive } from 'vue'
import { QueryClient, VueQueryPlugin } from '@tanstack/vue-query'
import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { describe, expect, it } from 'vitest'

import { emptyForm } from '../lib/form/form-state'
import { useTxPricing } from './use-tx-pricing'
import type { TxFormState } from './use-tx-form-state'

const asset = (symbol: string): AssetRef =>
  ({ symbol, name: symbol, coingecko_id: null, chain: null, contract: null }) as AssetRef

function setup(editing: boolean) {
  const form = reactive({ ...emptyForm('s1', 'payment'), asset: asset('RUB'), price: '0.0125', priceCurrency: 'usd' })
  const state = { props: { template: true }, form, editing: { value: editing } } as unknown as TxFormState
  const host = defineComponent({ setup: () => useTxPricing(state), template: '<div />' })
  mount(host, { global: { plugins: [createPinia(), [VueQueryPlugin, { queryClient: new QueryClient() }]] } })
  return form
}

describe('useTxPricing: asset change', () => {
  it('clears the price and its currency on an edited form', async () => {
    const form = setup(true)
    form.asset = asset('USD')
    await nextTick()
    expect([form.price, form.priceCurrency]).toEqual(['', ''])
  })

  it('keeps a price set in the same tick as the asset', async () => {
    const form = setup(true)
    form.price = '90'
    form.asset = asset('USD')
    await nextTick()
    expect(form.price).toBe('90')
  })

  it('clears a price typed for the current asset on the next asset change', async () => {
    const form = setup(true)
    form.asset = asset('USD')
    await nextTick()
    form.price = '90'
    await nextTick()
    form.asset = asset('EUR')
    await nextTick()
    expect([form.price, form.priceCurrency]).toEqual(['', ''])
  })

  it('clears on a new form too', async () => {
    const form = setup(false)
    form.asset = asset('USD')
    await nextTick()
    expect([form.price, form.priceCurrency]).toEqual(['', ''])
  })
})
