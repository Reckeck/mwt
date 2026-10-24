<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AssetChip } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { formatAsOf, formatMoney, formatPct } from '@/shared/lib/format'
import { toIsoTimestamp } from '@/shared/lib/datetime'
import { accountStoreId } from '@/shared/lib/transactions'
import { useSettingsStore } from '@/shared/store/settings'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'
import { AvailableHint } from '../available-hint'

/**
 * Exchange in the short form (OpForm-1 · 6): what is given and what is got,
 * each with its currency as a chip, then from which wallet into which. The rate
 * follows from the two amounts and is set against the market at the operation's
 * time (Currency-1 · 6); P2P, several accounts and a typed rate stay in the full editor.
 */
const { t } = useI18n()
const { form, check, preview } = useTxForm()
const settings = useSettingsStore()
const fromStore = computed(() => accountStoreId(form.from))

/** «Курс сделки 93,75 ₽ за USDT»: the price of one received unit in the given currency. */
const rate = computed(() => {
  const give = Number(form.fromAmount.replace(/\s/g, '').replace(',', '.'))
  const got = Number(form.toAmount.replace(/\s/g, '').replace(',', '.'))
  if (!(give > 0 && got > 0) || !form.fromAsset || !form.toAsset) return ''
  return t('tx.quick.dealPrice', {
    price: formatMoney(give / got, { currency: form.fromAsset.symbol }),
    unit: form.toAsset.symbol,
  })
})
/** «Рынок 21 окт. 14:00 · 92,10 ₽» and «Переплата обменнику 805 ₽ · 8,60 $ · 1,8 %», from the core. */
const market = computed(() => {
  const convert = preview.data.value?.convert
  if (!rate.value || !convert || !form.fromAsset) return null
  const currency = form.fromAsset.symbol
  const pct = Number(convert.overpay_pct)
  // Below the market the exchanger paid extra: the same line reads as a gain, in plain numbers.
  const abs = (value: string) => String(Math.abs(Number(value)))
  return {
    at: formatAsOf(toIsoTimestamp(form.timestamp) ?? new Date().toISOString()),
    price: formatMoney(convert.market_price, { currency }),
    overpay: [
      formatMoney(abs(convert.overpay), { currency, decimals: 0 }),
      formatMoney(abs(convert.overpay_base), { currency: settings.baseCurrency.toUpperCase() }),
      formatPct(Math.abs(pct), { decimals: 1 }),
    ].join(' · '),
    // A point over the market is the exchanger's usual cut; past it the line is worth a look.
    high: pct > 1,
    gain: pct < 0,
  }
})
const giveError = computed(() => (check.error('fromAmount') ? t('form.amountError') : check.error('fromAsset')))
const getError = computed(() => (check.error('toAmount') ? t('form.amountError') : check.error('toAsset')))
</script>

<template>
  <div class="grid items-start gap-3 @md:grid-cols-2">
    <FormField for="tx-amount" :label="t('tx.field.give')" required :error="giveError">
      <AmountInput id="tx-amount" v-model="form.fromAmount" size="lg" :placeholder="t('form.amountZero')">
        <template #suffix>
          <AssetChip v-model="form.fromAsset" :label="t('form.asset')" :invalid="Boolean(check.error('fromAsset'))" />
        </template>
      </AmountInput>
      <AvailableHint :store-id="fromStore" :asset="form.fromAsset" @max="(quantity) => (form.fromAmount = quantity)" />
    </FormField>
    <FormField for="tx-to-amount" :label="t('tx.field.receive')" required :error="getError">
      <AmountInput id="tx-to-amount" v-model="form.toAmount" size="lg" :placeholder="t('form.amountZero')">
        <template #suffix>
          <AssetChip v-model="form.toAsset" :label="t('form.asset')" :invalid="Boolean(check.error('toAsset'))" />
        </template>
      </AmountInput>
    </FormField>
  </div>

  <div class="grid items-start gap-3 @md:grid-cols-2">
    <FormField :label="t('tx.quick.fromWallet')" required :error="check.error('from')">
      <AccountRefPicker v-model="form.from" :contacts="false" />
    </FormField>
    <FormField :label="t('tx.quick.toWallet')" required :error="check.error('to')">
      <AccountRefPicker v-model="form.to" :contacts="false" :exclude-store-id="fromStore" />
    </FormField>
  </div>

  <div v-if="rate" class="grid gap-1.5 rounded-md bg-muted px-3 py-2.5 text-sm" data-tx-rate>
    <p class="flex justify-between gap-3">
      <span>{{ t('tx.quick.dealRate') }}</span><b class="font-semibold tabular-nums">{{ rate }}</b>
    </p>
    <template v-if="market">
      <p class="flex justify-between gap-3 text-muted-foreground">
        <span>{{ t('tx.quick.market', { at: market.at }) }}</span><span class="tabular-nums">{{ market.price }}</span>
      </p>
      <p class="flex justify-between gap-3" data-tx-overpay>
        <span>{{ t(market.gain ? 'tx.quick.underpay' : 'tx.quick.overpay') }}</span>
        <b class="text-right font-semibold tabular-nums" :class="market.high ? 'text-warning' : market.gain && 'text-success'">{{ market.overpay }}</b>
      </p>
    </template>
  </div>
</template>
