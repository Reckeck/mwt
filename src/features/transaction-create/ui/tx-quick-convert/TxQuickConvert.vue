<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'

import { AssetChip } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { formatMoney } from '@/shared/lib/format'
import { accountStoreId } from '@/shared/lib/transactions'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'
import { AvailableHint } from '../available-hint'

/**
 * Exchange in the short form (OpForm-1 · 6): what is given and what is got,
 * each with its currency as a chip, then from which wallet into which. The rate
 * follows from the two amounts; P2P, several accounts and a typed rate stay in
 * the full editor.
 */
const { t } = useI18n()
const { form, check } = useTxForm()
const fromStore = computed(() => accountStoreId(form.from))

/** «Курс 93,75 ₽ за USDT — посчитан сам»: the price of one received unit in the given currency. */
const rate = computed(() => {
  const give = Number(form.fromAmount.replace(/\s/g, '').replace(',', '.'))
  const got = Number(form.toAmount.replace(/\s/g, '').replace(',', '.'))
  if (!(give > 0 && got > 0) || !form.fromAsset || !form.toAsset) return ''
  return t('tx.quick.rate', {
    price: formatMoney(give / got, { currency: form.fromAsset.symbol }),
    unit: form.toAsset.symbol,
  })
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

  <p v-if="rate" class="flex items-center gap-2 rounded-md bg-muted px-3 py-2.5 text-sm" data-tx-rate>
    <Check class="size-4 shrink-0" />{{ rate }}
  </p>
</template>
