<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { CurrencyPicker } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'

import { useStoreCreate } from '../../model/use-store-create-state'

/** The money a new wallet starts with: a bank's first card, and what is already there. */
const { t } = useI18n()
const {
  name,
  withCard,
  cardCurrency,
  cardName,
  offersOpening,
  openingAmount,
  openingPicker,
  openingCurrency,
  openingToken,
  openingSide,
  openingAsset,
  pair,
  form,
} = useStoreCreate()
</script>

<template>
  <!-- A bank always has a card: it is created with the bank. -->
  <FormField
    v-if="withCard"
    for="store-card-currency"
    :label="t('storeWizard.card')"
    required
    :error="form.error('cardCurrency')"
    :hint="t('storeWizard.cardHint')"
    data-tour="store-bank-card"
  >
    <div class="grid grid-cols-[10rem_minmax(0,1fr)] gap-2">
      <CurrencyPicker id="store-card-currency" v-model="cardCurrency" :kinds="['fiat']" label="" />
      <Input
        v-model="cardName"
        autocomplete="off"
        :placeholder="t('storeWizard.cardNamePlaceholder', { name: `${name.trim() || t('stores.kind.bank')} ${cardCurrency.toUpperCase()}` })"
        :aria-label="t('storeWizard.cardName')"
      />
    </div>
  </FormField>

  <!-- Optional: money already there, so the new wallet does not start at zero. -->
  <FormField
    v-if="offersOpening"
    for="store-opening-amount"
    :label="t('storeWizard.opening')"
    :error="form.error('opening')"
    :hint="t('storeWizard.openingHint')"
    data-tour="store-opening"
  >
    <div class="grid grid-cols-[minmax(0,1fr)_10rem] gap-2">
      <AmountInput id="store-opening-amount" v-model="openingAmount" autocomplete="off" :placeholder="t('storeWizard.openingPlaceholder')" />
      <CurrencyPicker v-if="openingPicker === 'currency'" v-model="openingCurrency" />
      <AssetInput v-else-if="openingPicker === 'asset'" v-model="openingToken" />
      <Select v-else-if="openingPicker === 'side' && pair" v-model="openingSide" :aria-label="t('binding.pair')">
        <SelectItem value="base">{{ pair.base }}</SelectItem>
        <SelectItem value="quote">{{ pair.quote }}</SelectItem>
      </Select>
      <span v-else-if="openingAsset" class="flex h-9 items-center px-1 text-sm text-muted-foreground">
        {{ openingAsset.symbol }}
      </span>
    </div>
  </FormField>
</template>
