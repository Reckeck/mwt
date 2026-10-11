<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'

import { LIABILITY_KINDS } from './liability-terms-fields.constants'

/** Loan terms of a `liability` wallet; empty inputs are `null`. */
const terms = defineModel<LiabilityTerms>({ required: true })

const { t } = useI18n()

const text = (value: string | number | null) => (value === null ? '' : String(value))

function setAmount(key: 'rate' | 'limit' | 'monthly_payment', raw: string | number) {
  const value = String(raw).trim().replace(',', '.')
  terms.value = { ...terms.value, [key]: value || null }
}

function setDay(raw: string | number) {
  const day = Number.parseInt(String(raw), 10)
  terms.value = { ...terms.value, due_day: Number.isFinite(day) ? day : null }
}
</script>

<template>
  <div class="grid gap-4 @sm:grid-cols-2">
    <div class="grid gap-1.5">
      <Label for="liability-kind">{{ t('liability.kind') }}</Label>
      <Select id="liability-kind" :model-value="terms.kind" @update:model-value="(kind) => (terms = { ...terms, kind: kind as LiabilityKind })">
        <option v-for="kind in LIABILITY_KINDS" :key="kind" :value="kind">{{ t(`liabilityKind.${kind}`) }}</option>
      </Select>
    </div>
    <div class="grid gap-1.5">
      <Label for="liability-rate">{{ t('liability.rate') }}</Label>
      <AmountInput id="liability-rate" :model-value="text(terms.rate)" @update:model-value="(value) => setAmount('rate', value)" />
    </div>
    <div class="grid gap-1.5">
      <Label for="liability-limit">{{ t('liability.limit') }}</Label>
      <AmountInput id="liability-limit" :model-value="text(terms.limit)" @update:model-value="(value) => setAmount('limit', value)" />
    </div>
    <div class="grid gap-1.5">
      <Label for="liability-payment">{{ t('liability.monthlyPayment') }}</Label>
      <AmountInput
        id="liability-payment"
        :model-value="text(terms.monthly_payment)"
        @update:model-value="(value) => setAmount('monthly_payment', value)"
      />
    </div>
    <div class="grid gap-1.5">
      <Label for="liability-day">{{ t('liability.dueDay') }}</Label>
      <Input id="liability-day" inputmode="numeric" :model-value="text(terms.due_day)" @update:model-value="setDay" />
    </div>
  </div>
</template>
