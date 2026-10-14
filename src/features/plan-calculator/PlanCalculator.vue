<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { positiveAmount, useFormValidation } from '@/shared/lib/form'
import { cn } from '@/shared/lib/utils'

import { type ResultRow, resultRows } from './lib/result-rows'
import { useCalcPreview } from './model/use-calc-preview'
import type { CalculatorPlan, PlanCalculatorProps } from './plan-calculator.types'

const props = defineProps<PlanCalculatorProps>()

const emit = defineEmits<{
  /** Hand a calculated plan over to the transaction form. */
  apply: [payload: CalculatorPlan]
}>()

const { t } = useI18n()

const { mode, from, to, amount, priceIn, priceOut, targetPrice, stopPrice, feePct, result, error, impliedRate } =
  useCalcPreview()

const rows = computed<ResultRow[]>(() =>
  result.value ? resultRows(result.value, mode.value, props.baseCurrency, to.value?.symbol, t) : [],
)

const form = useFormValidation(() => ({ amount: positiveAmount(amount.value), priceIn: positiveAmount(priceIn.value) }))

const CALC_MODES: CalcMode[] = ['converter', 'position']

function pickMode(value: unknown) {
  if (value === 'converter' || value === 'position') mode.value = value
}

/** Checks the two fields a plan needs, then hands the plan over. */
function apply(event?: Event) {
  form.submit(emitPlan, event)
}

function emitPlan() {
  emit('apply', {
    mode: mode.value,
    from: from.value,
    to: to.value,
    amount: amount.value.trim(),
    price: priceIn.value.trim(),
  })
}

/** The dialog owns the "apply" button, so it reads these. */
defineExpose({ apply })
</script>

<template>
  <div class="flex flex-col gap-4">
    <ToggleGroup
      type="single"
      variant="outline"
      size="sm"
      :model-value="mode"
      :aria-label="t('calc.title')"
      @update:model-value="pickMode"
    >
      <ToggleGroupItem v-for="tab in CALC_MODES" :key="tab" :value="tab">
        {{ t(`calc.mode.${tab}`) }}
      </ToggleGroupItem>
    </ToggleGroup>

    <div class="grid gap-3 @sm:grid-cols-2">
      <AssetInput v-model="from" :label="t('calc.from')" :placeholder="t('asset.searchPlaceholder')" />
      <AssetInput v-if="mode === 'converter'" v-model="to" :label="t('calc.to')" :placeholder="t('asset.searchPlaceholder')" />
      <div v-else class="grid gap-1.5">
        <Label>{{ t('calc.targetPrice') }}</Label>
        <AmountInput v-model="targetPrice" :placeholder="t('calc.targetPriceHint')" />
      </div>
    </div>

    <div class="grid gap-3 @sm:grid-cols-3">
      <FormField
        for="calc-amount"
        :label="t(mode === 'converter' ? 'calc.amount' : 'calc.quantity')"
        required
        :error="form.error('amount')"
      >
        <AmountInput id="calc-amount" v-model="amount" :placeholder="t('calc.amountHint')" />
      </FormField>
      <FormField
        for="calc-price-in"
        :label="t(mode === 'converter' ? 'calc.priceIn' : 'calc.entryPrice')"
        required
        :error="form.error('priceIn')"
      >
        <AmountInput id="calc-price-in" v-model="priceIn" placeholder="0" />
      </FormField>
      <div class="grid gap-1.5">
        <Label>{{ t(mode === 'converter' ? 'calc.priceOut' : 'calc.stopPrice') }}</Label>
        <AmountInput v-if="mode === 'converter'" v-model="priceOut" placeholder="0" />
        <AmountInput v-else v-model="stopPrice" :placeholder="t('calc.stopPriceHint')" />
      </div>
    </div>

    <div class="grid gap-1.5">
      <Label>{{ t('calc.feePct') }}</Label>
      <AmountInput v-model="feePct" placeholder="0.1" class="max-w-28" />
    </div>

    <p v-if="impliedRate && mode === 'converter'" class="text-xs text-muted-foreground">
      {{ t('calc.rate') }}: 1 {{ from?.symbol.toUpperCase() || '—' }} ≈ {{ impliedRate }}
      {{ to?.symbol.toUpperCase() || '' }}
    </p>

    <div v-if="rows.length" class="flex flex-col gap-1.5 rounded-md border border-border bg-muted/40 p-3">
      <div v-for="row in rows" :key="row.key" class="flex items-baseline justify-between gap-3 text-sm">
        <span class="text-muted-foreground">{{ row.label }}</span>
        <span
          class="truncate font-medium tabular-nums"
          :class="cn(
            row.emphasis && 'text-base',
            row.tone === 'positive' && 'text-success',
            row.tone === 'negative' && 'text-destructive',
          )"
        >
          {{ row.value }}
        </span>
      </div>
    </div>

    <p v-if="error" class="text-xs text-destructive">{{ error }}</p>
  </div>
</template>
