import { computed, type MaybeRefOrGetter, toValue } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

/** `AssetRef::key()` in Rust for a thing with no chain or contract. */
export const thingKey = (asset: AssetRef) => `asset:${asset.symbol}`

/** A valuation moves the value from its date on, so every value view is stale. */
export function useValuationRefresh() {
  const queryClient = useQueryClient()
  return () => {
    for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS, ['property-valuations']])
      void queryClient.invalidateQueries({ queryKey })
  }
}

/** The value story of every asset of one property. */
export function usePropertyValues(storeId: MaybeRefOrGetter<string>) {
  return useQuery({
    // no placeholder: another property's values
    queryKey: computed(() => ['property-valuations', toValue(storeId)]),
    queryFn: () => call<PropertyValue[]>('property_valuations', { storeId: toValue(storeId) }),
  })
}

export function useValuationMutations(onSaved: () => void) {
  const refresh = useValuationRefresh()

  const add = useMutation({
    mutationFn: (input: ValuationInput) => call<Valuation>('property_valuation_add', { input }),
    onSuccess: () => {
      refresh()
      onSaved()
    },
    meta: { error: 'asset.revalueFailed', success: 'asset.revalued' },
  })

  const remove = useMutation({
    mutationFn: (id: string) => call<Valuation>('property_valuation_delete', { id }),
    onSuccess: refresh,
    meta: {
      error: 'valuation.deleteFailed',
      success: 'valuation.deleted',
      undo: async (data) => {
        const { id: _id, ...input } = data as Valuation
        await call<Valuation>('property_valuation_add', { input })
        refresh()
      },
    },
  })

  const reset = useMutation({
    mutationFn: (assetKey: string) => call('clear_manual_price', { assetKey }),
    onSuccess: refresh,
    meta: { error: 'asset.revalueFailed', success: 'asset.revalueCleared' },
  })

  return { add, remove, reset }
}
