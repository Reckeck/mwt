<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { formatMoney } from '@/shared/lib/format'
import type { FormValidation } from '@/shared/lib/form'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { SwitchRow } from '@/shared/ui/switch-row'

import { BUDGET_WARN_PERCENT } from '../budget-edit.constants'
import type { BudgetField, BudgetSpending } from '../budget-edit.types'

/**
 * Step 2 (Money 1 · 4): how much, with the category's usual month and how much
 * of the new limit the period already took.
 */
const props = defineProps<{ form: FormValidation<BudgetField>; period: BudgetPeriod; spending: BudgetSpending | null }>()
const limit = defineModel<string>('limit', { required: true })
const currency = defineModel<string>('currency', { required: true })
const warn = defineModel<boolean>('warn', { required: true })

const { t } = useI18n()
const money = (value: string, code = props.spending?.currency) => formatMoney(value, { decimals: 0, currency: code })

/** «36 из 50 $ · 72 %»: only a monthly limit in the spending's own currency compares. */
const taken = computed(() => {
  const spending = props.spending
  const cap = Number(limit.value)
  if (!spending || props.period !== 'month' || !(cap > 0) || currency.value.toLowerCase() !== spending.currency.toLowerCase()) return null
  const percent = Math.round((Number(spending.spent) / cap) * 100)
  return t('budget.takenOf', { spent: money(spending.spent).replace(/\s*\D+$/, ''), limit: money(limit.value), percent })
})
</script>

<template>
  <div class="grid gap-4">
    <FormField for="budget-limit" :label="t(`budget.howMuch.${props.period}`)" required :error="props.form.error('limit') ?? props.form.error('currency')">
      <AmountInput id="budget-limit" v-model="limit" size="lg">
        <template #suffix>
          <CurrencyPicker id="budget-currency" v-model="currency" label="" compact :error="props.form.error('currency')" class="h-9 w-24" />
        </template>
      </AmountInput>
      <p v-if="props.spending" class="text-xs text-muted-foreground">
        <template v-if="props.spending.average">{{ t('budget.usual', { average: money(props.spending.average) }) }} · </template>
        {{ props.spending.when }} {{ money(props.spending.spent) }}
      </p>
    </FormField>

    <div v-if="taken" class="flex items-center justify-between gap-3 rounded-lg bg-muted px-3 py-2.5 text-sm">
      <span>{{ t('budget.takenNow') }}</span>
      <b class="tabular-nums">{{ taken }}</b>
    </div>

    <SwitchRow id="budget-warn" v-model="warn" :label="t('budget.warnAt', { percent: BUDGET_WARN_PERCENT })" />
  </div>
</template>
