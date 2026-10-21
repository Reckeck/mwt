import { ref, watch, type Ref } from 'vue'

import { firstError, integerInRange, required, useFormValidation } from '@/shared/lib/form'

import { useFundSet } from './use-fund-edit'

export const FUND_MONTHS_MAX = 24

/** The fund dialog's fields, filled from the current fund each time it opens. */
export function useFundForm(fund: () => EmergencyFund, open: Ref<boolean>) {
  const storeIds = ref<string[]>([])
  const targetMonths = ref('6')
  const avgMonths = ref('6')
  watch(open, (isOpen) => {
    if (!isOpen) return
    storeIds.value = [...fund().scope.store_ids]
    targetMonths.value = String(fund().target_months)
    avgMonths.value = String(fund().avg_months)
  })

  const months = (value: string) => firstError(required(value), integerInRange(value, 1, FUND_MONTHS_MAX))
  const form = useFormValidation(() => ({ target: months(targetMonths.value), avg: months(avgMonths.value) }), {
    resetOn: open,
  })
  const set = useFundSet()

  function save() {
    const next: EmergencyFund = {
      scope: { store_ids: storeIds.value, include_children: false },
      target_months: Number(targetMonths.value),
      avg_months: Number(avgMonths.value),
    }
    set.mutate({ fund: next, previous: fund() }, { onSuccess: () => (open.value = false) })
  }

  return { storeIds, targetMonths, avgMonths, form, set, save }
}
