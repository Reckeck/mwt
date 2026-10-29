import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import type { TxFormState } from './use-tx-form-state'

/** Budget limits this payment would break (plan §9.7), said before saving. */
export function useTxBudget(state: TxFormState) {
  const { t } = useI18n()
  const { props, form } = state

  const budgetStatus = useQuery({
    queryKey: ['budget-status'],
    queryFn: () => call<BudgetStatus[]>('get_budget_status'),
    enabled: computed(() => !props.template && form.kind === 'payment'),
  })
  const overLimits = useQuery({
    queryKey: computed(() => [
      'budget-overrun',
      form.kind,
      form.amount,
      form.asset?.symbol,
      form.categories.join(','),
      budgetStatus.data.value,
    ]),
    placeholderData: keepPreviousData,
    queryFn: async () => {
      const amount = Number(form.amount)
      const asset = form.asset
      if (!asset || !(amount > 0)) return []
      const matching = (budgetStatus.data.value ?? []).filter((status) => {
        const target = status.budget.target
        if (target === 'total') return true
        if ('category' in target) return form.categories.includes(target.category)
        return target.store.store_ids.includes(props.storeId)
      })
      const out: Array<{ status: BudgetStatus; over: number }> = []
      for (const status of matching) {
        const converted = await call<ConvertedAmount>('convert_amount', {
          input: { from: asset.symbol, to: status.budget.currency, amount: form.amount, at: null },
        })
        const over = Number(converted.amount ?? 0) - Number(status.remaining)
        if (converted.amount !== null && over > 0) out.push({ status, over })
      }
      return out
    },
    enabled: computed(() => !props.template && form.kind === 'payment' && Boolean(budgetStatus.data.value?.length)),
  })
  /** `The monthly limit will be exceeded by 120 $`, one per budget the sum does not fit: the parts of each. */
  const overruns = computed(() =>
    (overLimits.data.value ?? []).map(({ status, over }) => ({
      id: status.budget.id,
      period: t(`budget.periodKind.${status.budget.period}`),
      amount: over,
      currency: status.budget.currency,
    })),
  )

  return { overruns }
}
