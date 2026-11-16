<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { USDT } from '@/entities/asset'
import { Money } from '@/entities/currency'
import { positiveAmount, useFormValidation } from '@/shared/lib/form'
import { cn } from '@/shared/lib/utils'
import { Tabs } from '@/shared/ui/tabs'

import { type ResultRow, resultRows } from './lib/result-rows'
import { useCalcPreview } from './model/use-calc-preview'
import type { CalculatorPlan, PlanCalculatorProps } from './plan-calculator.types'
import ConverterFields from './ui/ConverterFields.vue'
import PositionFields from './ui/PositionFields.vue'

const props = defineProps<PlanCalculatorProps>()

const emit = defineEmits<{
  /** Hand a calculated plan over to the transaction form. */
  apply: [payload: CalculatorPlan]
}>()

const { t } = useI18n()

const calc = useCalcPreview()
const { mode, from, to, asset, amount, priceIn, result, error } = calc

const rows = computed<ResultRow[]>(() =>
  result.value ? resultRows(result.value, mode.value, props.baseCurrency, to.value?.symbol, t) : [],
)

const form = useFormValidation(() => ({ amount: positiveAmount(amount.value), priceIn: positiveAmount(priceIn.value) }))

/** `Position` first: the deal question the frame opens with. */
const tabs = computed(() =>
  (['position', 'converter'] as const).map((value) => ({ value, label: t(`calc.mode.${value}`) })),
)

function pickMode(value: string) {
  if (value === 'converter' || value === 'position') mode.value = value
}

/** Checks the two fields a plan needs, then hands the plan over. */
function apply(event?: Event) {
  form.submit(emitPlan, event)
}

function emitPlan() {
  if (mode.value === 'position') {
    // An exchange of USDT for the coin: the amount given and the units it buys, the same sums as the calculation.
    emit('apply', {
      mode: 'position',
      from: { ...USDT },
      to: asset.value,
      amount: amount.value.trim(),
      quantity: result.value?.quantity,
      entryPrice: priceIn.value.trim(),
      price: '',
    })
    return
  }
  emit('apply', { mode: 'converter', from: from.value, to: to.value, amount: amount.value.trim(), price: priceIn.value.trim() })
}

/** Both sides are known (always in `position`, in `converter` once the second asset is chosen): the plan becomes an exchange, else a top-up. */
const exchange = computed(() => mode.value === 'position' || Boolean(to.value))

/** The dialog owns the "apply" button, so it reads these. */
defineExpose({ apply, exchange })
</script>

<template>
  <div class="flex flex-col gap-4" data-trade-calc>
    <Tabs :model-value="mode" :tabs="tabs" @update:model-value="pickMode">
      <template #position>
        <PositionFields :calc="calc" :amount-error="form.error('amount')" :price-error="form.error('priceIn')" />
      </template>
      <template #converter>
        <ConverterFields :calc="calc" :amount-error="form.error('amount')" :price-error="form.error('priceIn')" />
      </template>
    </Tabs>

    <div v-if="rows.length" class="flex flex-col gap-1.5 rounded-lg bg-muted p-3" data-trade-calc-result>
      <div v-for="row in rows" :key="row.key" class="flex items-baseline justify-between gap-3 text-sm">
        <span class="text-muted-foreground">{{ row.label }}</span>
        <span
          class="truncate font-semibold tabular-nums"
          :class="cn(row.tone === 'positive' && 'text-success', row.tone === 'negative' && 'text-destructive')"
        >
          <template v-for="(part, at) in row.value" :key="at">
            <Money v-if="'amount' in part" v-bind="part" />
            <template v-else>{{ part.text }}</template>
          </template>
        </span>
      </div>
    </div>

    <p v-if="error" class="text-xs text-destructive">{{ error }}</p>
  </div>
</template>
