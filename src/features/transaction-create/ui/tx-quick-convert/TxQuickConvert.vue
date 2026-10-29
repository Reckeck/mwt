<script setup lang="ts">
import { computed, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'

import { AssetChip } from '@/entities/asset'
import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { formatAsOf, formatPct } from '@/shared/lib/format'
import { toIsoTimestamp } from '@/shared/lib/datetime'
import { accountStoreId } from '@/shared/lib/transactions'
import { useSettingsStore } from '@/shared/store/settings'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'
import { convertAsTransfer } from '../../lib/form'
import { AvailableHint } from '../available-hint'

/**
 * Exchange in the short form (OpForm-1 · 6): what is given and what is got,
 * each with its currency as a chip, then from which wallet into which. The rate
 * follows from the two amounts and is set against the market at the operation's
 * time (Currency-1 · 6); P2P, several accounts and a typed rate stay in the full editor.
 */
const { t } = useI18n()
const { form, check, preview, sameCurrency, noteExpression } = useTxForm()
const settings = useSettingsStore()
const fromStore = computed(() => accountStoreId(form.from))

/** `Deal rate 93.75 ₽ per USDT`: the price of one received unit in the given currency. */
const rate = computed(() => {
  const give = Number(form.fromAmount.replace(/\s/g, '').replace(',', '.'))
  const got = Number(form.toAmount.replace(/\s/g, '').replace(',', '.'))
  if (!(give > 0 && got > 0) || !form.fromAsset || !form.toAsset) return null
  return { price: give / got, currency: form.fromAsset.symbol, unit: form.toAsset.symbol }
})
/** `Market Oct 21 14:00 · 92.10 ₽` and `Overpaid to the exchanger 805 ₽ · 8.60 $ · 1.8 %`, from the core. */
const market = computed(() => {
  const convert = preview.data.value?.convert
  if (!rate.value || !convert || !form.fromAsset) return null
  const currency = form.fromAsset.symbol
  const pct = Number(convert.overpay_pct)
  // Below the market the exchanger paid extra: the same line reads as a gain, in plain numbers.
  const abs = (value: string) => String(Math.abs(Number(value)))
  return {
    at: formatAsOf(toIsoTimestamp(form.timestamp) ?? new Date().toISOString()),
    price: convert.market_price,
    currency,
    overpay: abs(convert.overpay),
    overpayBase: abs(convert.overpay_base),
    pct: formatPct(Math.abs(pct), { decimals: 1 }),
    // A point over the market is the exchanger's usual cut; past it the line is worth a look.
    // The same currency as the base one: the sum is told once, not again in other rounding.
    sameAsBase: currency.toLowerCase() === settings.baseCurrency.toLowerCase(),
    high: pct > 1,
    gain: pct < 0,
  }
})
const giveError = computed(() => (check.error('fromAmount') ? t('form.amountError') : check.error('fromAsset')))
const getError = computed(() => (check.error('toAmount') ? t('form.amountError') : check.error('toAsset')))
/** One currency on both sides is not a mistake, it is a transfer: a plain hint as soon as both are chosen (red only after a save was tried). */
const sameHint = computed(() => (sameCurrency.value ? t('validation.sameCurrency') : undefined))
/** Same currency is a move between wallets: the form goes on as `Transfer` with what was typed. */
function recordAsTransfer() {
  convertAsTransfer(form)
  void nextTick(() => document.getElementById('tx-amount')?.focus())
}
</script>

<template>
  <div class="grid items-start gap-3 @md:grid-cols-2">
    <FormField for="tx-amount" :label="t('tx.field.give')" required :error="giveError">
      <AmountInput id="tx-amount" @expression="noteExpression('tx-amount', $event)" v-model="form.fromAmount" size="lg" :placeholder="t('form.amountZero')">
        <template #suffix>
          <AssetChip v-model="form.fromAsset" :label="t('form.asset')" :invalid="Boolean(check.error('fromAsset'))" />
        </template>
      </AmountInput>
      <AvailableHint :store-id="fromStore" :asset="form.fromAsset" @max="(quantity) => (form.fromAmount = quantity)" />
    </FormField>
    <div class="grid content-start gap-1.5">
      <FormField for="tx-to-amount" :label="t('tx.field.receive')" required :error="getError" :hint="sameHint">
        <AmountInput id="tx-to-amount" @expression="noteExpression('tx-to-amount', $event)" v-model="form.toAmount" size="lg" :placeholder="t('form.amountZero')">
          <template #suffix>
            <AssetChip
              v-model="form.toAsset"
              :label="t('form.asset')"
              :invalid="Boolean(check.error('toAsset'))"
            />
          </template>
        </AmountInput>
      </FormField>
      <Button v-if="sameCurrency" type="button" variant="link" class="touch-min h-auto justify-self-start px-0 py-0" data-tx-as-transfer @click="recordAsTransfer">
        {{ t('tx.quick.recordAsTransfer') }}
      </Button>
    </div>
  </div>

  <div class="grid items-start gap-3 @md:grid-cols-2">
    <FormField :label="t('tx.quick.fromWallet')" required :error="check.error('from')">
      <AccountRefPicker v-model="form.from" :contacts="false" />
    </FormField>
    <FormField :label="t('tx.quick.toWallet')" required :error="check.error('to')">
      <AccountRefPicker v-model="form.to" :contacts="false" :exclude-store-id="fromStore" />
    </FormField>
  </div>

  <!-- One currency has no rate: `1.00 THB per THB` says nothing. -->
  <div v-if="rate && !sameCurrency" class="grid gap-1.5 rounded-md bg-muted px-3 py-2.5 text-sm" data-tx-rate>
    <p class="flex justify-between gap-3">
      <span>{{ t('tx.quick.dealRate') }}</span><i18n-t keypath="tx.quick.dealPrice" tag="b" class="font-semibold tabular-nums">
        <template #price><Money :amount="rate.price" :currency="rate.currency" /></template>
        <template #unit>{{ rate.unit }}</template>
      </i18n-t>
    </p>
    <template v-if="market">
      <p class="flex justify-between gap-3 text-muted-foreground">
        <span>{{ t('tx.quick.market', { at: market.at }) }}</span><span class="tabular-nums"><Money :amount="market.price" :currency="market.currency" /></span>
      </p>
      <p class="flex justify-between gap-3" data-tx-overpay>
        <span>{{ t(market.gain ? 'tx.quick.underpay' : 'tx.quick.overpay') }}</span>
        <b class="text-right font-semibold tabular-nums" :class="market.high ? 'text-warning' : market.gain && 'text-success'">
          <template v-if="market.sameAsBase"><Money :amount="market.overpay" :currency="market.currency" /> · {{ market.pct }}</template>
          <template v-else>
            <Money :amount="market.overpay" :currency="market.currency" :decimals="0" /> ·
            <Money :amount="market.overpayBase" :currency="settings.baseCurrency" /> · {{ market.pct }}
          </template>
        </b>
      </p>
    </template>
  </div>
</template>
