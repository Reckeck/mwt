<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { FormField } from '@/shared/ui/form-field'

import type { CalcPreview } from '../model/use-calc-preview'

/** `Converter`: an amount of one asset priced into another, net of the commission. */
const props = defineProps<{
  calc: CalcPreview
  amountError?: string | null
  priceError?: string | null
}>()

const { t } = useI18n()
const { from, to, amount, priceIn, priceOut, feePct, impliedRate } = props.calc
</script>

<template>
  <div class="grid gap-4">
    <div class="grid grid-cols-2 items-start gap-3">
      <AssetInput v-model="from" :label="t('calc.from')" :placeholder="t('asset.searchPlaceholder')" />
      <AssetInput v-model="to" :label="t('calc.to')" :placeholder="t('asset.searchPlaceholder')" />
    </div>
    <div class="grid grid-cols-2 items-start gap-3">
      <FormField for="calc-amount" :label="t('calc.amount')" required :error="props.amountError ?? undefined">
        <AmountInput id="calc-amount" v-model="amount" :placeholder="t('calc.amountHint')" />
      </FormField>
      <FormField for="calc-fee" :label="t('calc.feeLabel')">
        <AmountInput id="calc-fee" v-model="feePct" placeholder="0,1">
          <template #suffix><Badge variant="secondary">%</Badge></template>
        </AmountInput>
      </FormField>
    </div>
    <div class="grid grid-cols-2 items-start gap-3">
      <FormField for="calc-price-in" :label="t('calc.priceIn')" required :error="props.priceError ?? undefined">
        <AmountInput id="calc-price-in" v-model="priceIn" placeholder="0" />
      </FormField>
      <FormField for="calc-price-out" :label="t('calc.priceOut')">
        <AmountInput id="calc-price-out" v-model="priceOut" placeholder="0" />
      </FormField>
    </div>
    <p v-if="impliedRate" class="text-xs text-muted-foreground">
      {{ t('calc.rate') }}: 1 {{ from?.symbol.toUpperCase() || '—' }} ≈ {{ impliedRate }}
      {{ to?.symbol.toUpperCase() || '' }}
    </p>
  </div>
</template>
