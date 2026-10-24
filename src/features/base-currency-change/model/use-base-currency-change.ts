// check-ui-ignore invalidate-all: a new base currency changes every figure on every screen
import { type MaybeRefOrGetter, type Ref, computed, ref, toValue, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { useConversions } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { useSettingsStore } from '@/shared/store/settings'

export type BudgetChoice = 'keep' | 'follow'

/**
 * «Сменить базовую валюту?» (Currency-2 · 9): what moves to the new currency
 * (the net worth, with today's figure in both), what stays, and whether the
 * budget follows. Nothing is written until the user confirms.
 */
export function useBaseCurrencyChange(target: MaybeRefOrGetter<string>, open: Ref<boolean>) {
  const { t } = useI18n()
  const settings = useSettingsStore()
  const queryClient = useQueryClient()
  const conversions = useConversions(open)

  const from = computed(() => settings.baseCurrency)
  const to = computed(() => toValue(target).toLowerCase())
  /** The budget's own currency; empty means «as the base», so it would move with it. */
  const own = computed(() => (settings.vault?.budget_currency || '').toLowerCase())
  const budget = computed(() => own.value || from.value)
  const askBudget = computed(() => budget.value !== to.value)
  const budgetChoice = ref<BudgetChoice>('keep')
  watch(open, (isOpen) => {
    if (isOpen) budgetChoice.value = 'keep'
  })

  const summary = useQuery({
    queryKey: ['summary'],
    queryFn: () => call<PortfolioSummary>('get_summary'),
    enabled: open,
    staleTime: 60_000,
  })
  /** «42 052,50 $ → 3 872 960 ₽» at today's rate; empty when the new currency has no rate. */
  const example = computed(() => {
    const worth = summary.data.value?.net_worth_base
    if (!worth) return ''
    const amount = Number(worth)
    const converted = conversions.convert(amount, from.value).find((entry) => entry.currency === to.value)
    if (!converted) return ''
    const before = formatMoney(worth, { currency: from.value, decimals: 2 })
    return `${before} → ${formatMoney(converted.value, { currency: to.value, decimals: 0 })}`
  })

  const save = useMutation({
    mutationFn: async () => {
      await settings.updateApp({ base_currency: to.value })
      if (!askBudget.value) return
      // «Оставить» pins a following budget to the old base; «Тоже» lets it follow the base again.
      if (budgetChoice.value === 'keep' && !own.value) await settings.updateVault({ budget_currency: from.value })
      if (budgetChoice.value === 'follow' && own.value) await settings.updateVault({ budget_currency: '' })
    },
    onSuccess: async () => {
      open.value = false
      await queryClient.invalidateQueries()
    },
    meta: {
      error: 'settings.saveFailed',
      success: () => t('baseChange.done', { code: to.value.toUpperCase() }),
    },
  })

  return { from, to, budget, askBudget, budgetChoice, example, save }
}
