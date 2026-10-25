<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { PairInput, resolveTicker } from '@/entities/asset'
import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { decimalAdd, decimalMul, decimalSub, decimalInput } from '@/shared/lib/decimal'
import { parsePairSymbol } from '@/shared/lib/pair'

import { useTxForm } from '../../model/use-tx-form'

type Side = 'buy' | 'sell'

/**
 * Spot in «Сделка · <биржа>» (OpForm-2 · 9): a side, a pair, how much of the
 * base at what price, the fee in the quote, and what leaves the wallet. A buy
 * sells the quote for the base, a sell the other way round; the full editor
 * keeps the order id and maker or taker.
 */
const { t } = useI18n()
const { form, check } = useTxForm()

const side = ref<Side>('buy')
/** A calculator plan or the previous trade of a series leaves its pair: bought base for the sold quote. */
const pair = ref(form.soldAsset && form.boughtAsset ? `${form.boughtAsset.symbol}${form.soldAsset.symbol}` : '')
const quantity = ref(form.boughtQty)
const price = ref(
  Number(form.boughtQty) > 0 ? decimalInput(Number(form.soldQty) / Number(form.boughtQty), 8) : '',
)
const parsed = computed(() => parsePairSymbol(pair.value))

const base = ref<AssetRef | null>(null)
const quote = ref<AssetRef | null>(null)
watch(
  parsed,
  async (next) => {
    if (!next) {
      base.value = quote.value = null
      return
    }
    const [baseRef, quoteRef] = await Promise.all([resolveTicker(next.base), resolveTicker(next.quote)])
    if (parsed.value?.base !== next.base || parsed.value?.quote !== next.quote) return
    base.value = baseRef
    quote.value = quoteRef
  },
  { immediate: true },
)

/** Quote paid or got before the fee. */
const cost = computed(() => (quantity.value && price.value ? decimalMul(quantity.value, price.value) : null))

watch(
  [side, base, quote, quantity, cost],
  () => {
    const buy = side.value === 'buy'
    form.soldAsset = buy ? quote.value : base.value
    form.boughtAsset = buy ? base.value : quote.value
    form.soldQty = (buy ? cost.value : quantity.value) ?? ''
    form.boughtQty = (buy ? quantity.value : cost.value) ?? ''
    form.feeAsset = quote.value
  },
  { immediate: true },
)

// The fee is in the quote only while this form is on screen.
onUnmounted(() => {
  form.feeAsset = null
})

/** «Итого спишется 1 286,30 USDT» on a buy, «Итого придёт …» on a sell. */
const total = computed(() => {
  if (!cost.value || !quote.value) return null
  const fee = form.feeAmount.trim() || '0'
  const sum = side.value === 'buy' ? decimalAdd(cost.value, fee) : decimalSub(cost.value, fee)
  return sum === null ? null : { amount: sum, currency: quote.value.symbol }
})

const pairError = computed(() => check.error('soldAsset') ?? check.error('boughtAsset'))
const quantityError = computed(() => check.error(side.value === 'buy' ? 'boughtQty' : 'soldQty'))
const priceError = computed(() =>
  quantity.value && !price.value ? check.error(side.value === 'buy' ? 'soldQty' : 'boughtQty') : undefined,
)
</script>

<template>
  <div class="grid gap-1.5">
    <Label>{{ t('tx.quick.side') }}</Label>
    <ToggleGroup
      type="single"
      variant="segmented"
      :spacing="1"
      class="justify-start"
      :model-value="side"
      :aria-label="t('tx.quick.side')"
      @update:model-value="(value) => value && (side = value as Side)"
    >
      <ToggleGroupItem value="buy" class="px-3" data-spot-side="buy">{{ t('tx.quick.buy') }}</ToggleGroupItem>
      <ToggleGroupItem value="sell" class="px-3" data-spot-side="sell">{{ t('tx.quick.sell') }}</ToggleGroupItem>
    </ToggleGroup>
  </div>

  <div class="grid items-start gap-3 @md:grid-cols-2">
    <FormField for="tx-pair" :label="t('tx.quick.pair')" required :error="pairError">
      <PairInput id="tx-pair" v-model="pair" placeholder="BTC / USDT" />
    </FormField>
    <FormField for="tx-amount" :label="t('tx.field.quantity')" required :error="quantityError">
      <AmountInput id="tx-amount" v-model="quantity" placeholder="0">
        <template v-if="parsed" #suffix><Badge variant="secondary">{{ parsed.base }}</Badge></template>
      </AmountInput>
    </FormField>
    <FormField for="tx-spot-price" :label="t('tx.field.price')" required :error="priceError">
      <AmountInput id="tx-spot-price" v-model="price" placeholder="0">
        <template v-if="parsed" #suffix><Badge variant="secondary">{{ parsed.quote }}</Badge></template>
      </AmountInput>
    </FormField>
    <FormField for="tx-fee" :label="t('form.fee')" :error="check.error('feeAmount')">
      <AmountInput id="tx-fee" v-model="form.feeAmount" placeholder="0">
        <template v-if="parsed" #suffix><Badge variant="secondary">{{ parsed.quote }}</Badge></template>
      </AmountInput>
    </FormField>
  </div>

  <p v-if="total" class="flex flex-wrap items-baseline justify-between gap-x-2 rounded-md bg-muted px-3 py-2.5 text-sm" data-spot-total>
    <span>{{ t(side === 'buy' ? 'tx.quick.totalSpent' : 'tx.quick.totalGot') }}</span>
    <b class="font-semibold tabular-nums"><Money :amount="total.amount" :currency="total.currency" /></b>
  </p>
</template>
