<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AssetInput, USDT } from '@/entities/asset'
import { formatAsOf } from '@/shared/lib/format'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { FormField } from '@/shared/ui/form-field'

import type { CalcPreview } from '../model/use-calc-preview'

/** «Позиция» (Money 4 · 23): USDT given for a coin at the entry price, the take and the stop. */
const props = defineProps<{
  calc: CalcPreview
  amountError?: string | null
  priceError?: string | null
}>()

const { t } = useI18n()
const { asset, amount, priceIn, targetPrice, stopPrice, feePct, result, priceAt } = props.calc

const quantity = computed(() => (result.value && Number(result.value.quantity) ? result.value.quantity : ''))
const coin = computed(() => asset.value?.symbol.toUpperCase() ?? '—')
</script>

<template>
  <div class="grid gap-3.5">
    <AssetInput v-model="asset" :label="t('calc.asset')" :placeholder="t('asset.searchPlaceholder')" data-calc-asset />
    <div class="grid grid-cols-2 items-start gap-3 @max-sm:grid-cols-1">
      <FormField for="calc-amount" :label="t('calc.from')" required :error="props.amountError ?? undefined">
        <AmountInput id="calc-amount" v-model="amount" placeholder="1 000">
          <template #suffix><Badge variant="secondary">{{ USDT.symbol }}</Badge></template>
        </AmountInput>
      </FormField>
      <FormField for="calc-quantity" :label="t('calc.to')">
        <AmountInput id="calc-quantity" :model-value="quantity" readonly tabindex="-1" placeholder="0">
          <template #suffix><Badge variant="secondary">{{ coin }}</Badge></template>
        </AmountInput>
      </FormField>
    </div>
    <div class="grid grid-cols-2 items-start gap-3 @max-sm:grid-cols-1">
      <FormField for="calc-price-in" :label="t('calc.entryPrice')" :error="props.priceError ?? undefined">
        <AmountInput id="calc-price-in" v-model="priceIn" placeholder="0">
          <template #suffix><Badge variant="secondary">{{ USDT.symbol }}</Badge></template>
        </AmountInput>
        <p v-if="priceAt" class="mt-1 text-xs text-muted-foreground" data-calc-price-at>
          {{ t('calc.priceAt', { at: formatAsOf(priceAt) }) }}
        </p>
      </FormField>
      <FormField for="calc-fee" :label="t('calc.feeLabel')">
        <AmountInput id="calc-fee" v-model="feePct" placeholder="0,1">
          <template #suffix><Badge variant="secondary">%</Badge></template>
        </AmountInput>
      </FormField>
    </div>
    <div class="grid grid-cols-2 items-start gap-3 @max-sm:grid-cols-1">
      <FormField for="calc-target" :label="t('calc.targetPrice')">
        <AmountInput id="calc-target" v-model="targetPrice" :placeholder="t('calc.targetPriceHint')">
          <template #suffix><Badge variant="secondary">{{ USDT.symbol }}</Badge></template>
        </AmountInput>
      </FormField>
      <FormField for="calc-stop" :label="t('calc.stopPrice')">
        <AmountInput id="calc-stop" v-model="stopPrice" :placeholder="t('calc.stopPriceHint')">
          <template #suffix><Badge variant="secondary">{{ USDT.symbol }}</Badge></template>
        </AmountInput>
      </FormField>
    </div>
  </div>
</template>
