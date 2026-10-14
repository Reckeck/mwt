import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'

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
  const overrunText = computed(() =>
    (overLimits.data.value ?? [])
      .map(({ status, over }) =>
        t('budget.overrunHint', {
          period: t(`budget.periodKind.${status.budget.period}`),
          amount: formatMoney(over, { currency: status.budget.currency }),
        }),
      )
      .join(' · '),
  )

  return { overrunText }
}
