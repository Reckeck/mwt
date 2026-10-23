<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { PairInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { formatMoney, formatQuantity } from '@/shared/lib/format'
import { parseDecimal, decimalInput } from '@/shared/lib/decimal'
import { parsePairSymbol } from '@/shared/lib/pair'

import { useTxForm } from '../../model/use-tx-form'

/**
 * Opening a futures position in «Сделка · <биржа>» (OpForm-2 · 10): long or
 * short, the contract, leverage, the margin put up and the entry price. The
 * position size follows from them, the liquidation price comes from the
 * backend's preview; closing a position stays in the full editor.
 */
const { t } = useI18n()
const { form, check, preview } = useTxForm()

form.action = 'open'
if (!form.leverage) form.leverage = '5'
const settle = computed(() => form.settle?.symbol.toUpperCase() ?? 'USDT')
const baseSymbol = computed(() => parsePairSymbol(form.symbol)?.base ?? '')

const margin = ref(
  Number(form.quantity) > 0 && Number(form.fillPrice) > 0 && Number(form.leverage) > 0
    ? decimalInput((Number(form.quantity) * Number(form.fillPrice)) / Number(form.leverage), 2)
    : '',
)
const num = (raw: string) => Number(parseDecimal(raw) ?? Number.NaN)

/** Position = margin × leverage ÷ entry price, in the contract's base. */
watch(
  [margin, () => form.leverage, () => form.fillPrice],
  () => {
    const size = (num(margin.value) * num(form.leverage)) / num(form.fillPrice)
    form.quantity = decimalInput(size, 8)
  },
  { immediate: true },
)

/** «Позиция 0,0223 BTC · ликвидация ≈ 72 100». */
const summary = computed(() => {
  if (!form.quantity) return null
  const position = `${formatQuantity(form.quantity, 4)} ${baseSymbol.value}`.trim()
  const liquidation = preview.data.value?.liquidation_price
  return {
    position,
    liquidation: liquidation ? formatMoney(Number(liquidation), { decimals: 0 }) : null,
  }
})
const marginError = computed(() => (margin.value ? undefined : check.error('quantity')))
</script>

<template>
  <div class="grid gap-1.5">
    <Label>{{ t('tx.quick.side') }}</Label>
    <ToggleGroup
      type="single"
      variant="segmented"
      :spacing="1"
      class="justify-start"
      :model-value="form.direction"
      :aria-label="t('tx.quick.side')"
      @update:model-value="(value) => value && (form.direction = value as FuturesDirection)"
    >
      <ToggleGroupItem value="long" class="px-3" data-futures-side="long">{{ t('tx.quick.long') }}</ToggleGroupItem>
      <ToggleGroupItem value="short" class="px-3" data-futures-side="short">{{ t('tx.quick.short') }}</ToggleGroupItem>
    </ToggleGroup>
  </div>

  <div class="grid items-start gap-3 @md:grid-cols-2">
    <FormField for="tx-symbol" :label="t('tx.field.contract')" required :error="check.error('symbol')">
      <PairInput id="tx-symbol" v-model="form.symbol" kind="futures" placeholder="BTCUSDT" />
    </FormField>
    <FormField for="tx-leverage" :label="t('tx.field.leverage')" :error="check.error('leverage')">
      <AmountInput id="tx-leverage" v-model="form.leverage" placeholder="1">
        <template #suffix><span class="px-2 text-sm text-muted-foreground">×</span></template>
      </AmountInput>
    </FormField>
    <FormField for="tx-amount" :label="t('tx.quick.margin')" required :error="marginError">
      <AmountInput id="tx-amount" v-model="margin" placeholder="0">
        <template #suffix><Badge variant="secondary">{{ settle }}</Badge></template>
      </AmountInput>
    </FormField>
    <FormField for="tx-fill" :label="t('tx.quick.entryPrice')" required :error="check.error('fillPrice')">
      <AmountInput id="tx-fill" v-model="form.fillPrice" placeholder="0">
        <template #suffix><Badge variant="secondary">{{ settle }}</Badge></template>
      </AmountInput>
    </FormField>
  </div>

  <p v-if="summary" class="flex flex-wrap items-baseline justify-between gap-x-2 rounded-md bg-muted px-3 py-2.5 text-sm" data-futures-summary>
    <span>{{ t('tx.quick.position') }}</span>
    <b class="font-semibold tabular-nums">
      {{ summary.position
      }}<template v-if="summary.liquidation"> · {{ t('tx.quick.liquidation', { price: summary.liquidation }) }}</template>
    </b>
  </p>
</template>
