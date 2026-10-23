import { computed, ref, watch, type Ref } from 'vue'

import { dateInputToIso, localDate } from '@/shared/lib/planning'
import { firstError, integerInRange, required, useFormValidation } from '@/shared/lib/form'

import { useFundGoal, useFundSet } from './use-fund-edit'

export const FUND_MONTHS_MAX = 24

/**
 * The fund dialog's fields, filled from the current fund each time it opens.
 * With the checkup's `status` it also sizes the fund in money and can open a goal for it.
 */
export function useFundForm(
  fund: () => EmergencyFund,
  status: () => EmergencyStatus | null | undefined,
  open: Ref<boolean>,
) {
  const storeIds = ref<string[]>([])
  const targetMonths = ref('6')
  const avgMonths = ref('6')
  const makeGoal = ref(false)
  watch(open, (isOpen) => {
    if (!isOpen) return
    storeIds.value = [...fund().scope.store_ids]
    targetMonths.value = String(fund().target_months)
    avgMonths.value = String(fund().avg_months)
    makeGoal.value = false
  })

  /** The sum the months stand for at today's spending, and what the accounts still lack. */
  const goal = computed(() => {
    const current = status()
    const months = Number(targetMonths.value)
    if (!current?.e_mand || !months) return null
    const amount = Math.round(Number(current.e_mand) * months)
    return {
      amount: String(amount),
      gap: String(Math.max(0, amount - Number(current.total))),
      currency: current.currency,
    }
  })

  const months = (value: string) => firstError(required(value), integerInRange(value, 1, FUND_MONTHS_MAX))
  const form = useFormValidation(
    () => ({
      target: months(targetMonths.value),
      avg: months(avgMonths.value),
      accounts: required(storeIds.value, 'pick'),
    }),
    { resetOn: open },
  )
  const set = useFundSet()
  const goalCreate = useFundGoal()

  function createGoal(name: string) {
    const sized = goal.value
    if (!makeGoal.value || !sized) return
    goalCreate.mutate({
      name,
      active: true,
      target_amount: sized.amount,
      currency: sized.currency,
      starts_at: dateInputToIso(localDate(new Date())),
      deadline: null,
      priority: 3,
      monthly_commitment: null,
      scope: { store_ids: storeIds.value, include_children: false },
      scenario_id: null,
      expected_return: null,
    })
  }

  function save(goalName: string) {
    const next: EmergencyFund = {
      scope: { store_ids: storeIds.value, include_children: false },
      target_months: Number(targetMonths.value),
      avg_months: Number(avgMonths.value),
    }
    set.mutate(
      { fund: next, previous: fund() },
      {
        onSuccess: () => {
          createGoal(goalName)
          open.value = false
        },
      },
    )
  }

  return { storeIds, targetMonths, avgMonths, makeGoal, goal, form, set, save }
}
