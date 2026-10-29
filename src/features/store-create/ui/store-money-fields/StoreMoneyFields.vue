<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { CurrencyPicker } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'

import { useStoreCreate } from '../../model/use-store-create-state'

/**
 * What the wallet holds now (Wallets-1 · 4): booked as `Opening balance` on
 * its day, not as income; and whether it goes to the sidebar right away.
 */
const { t } = useI18n()
const { isSub, openingAmount, openingPicker, openingCurrency, openingToken, openingSide, openingAsset, openingAt, favorite, pair, form } =
  useStoreCreate()
const today = new Date().toISOString().slice(0, 10)
</script>

<template>
  <FormField
    for="store-opening-amount"
    :label="t('storeWizard.opening')"
    :error="form.error('opening')"
    :hint="t('storeWizard.openingHint')"
    data-tour="store-opening"
  >
    <AmountInput id="store-opening-amount" v-model="openingAmount" size="lg" autocomplete="off" placeholder="0">
      <template #suffix>
        <CurrencyPicker v-if="openingPicker === 'currency'" v-model="openingCurrency" :kinds="['fiat']" label="" chip />
        <AssetInput v-else-if="openingPicker === 'asset'" v-model="openingToken" class="w-32" />
        <Select v-else-if="openingPicker === 'side' && pair" v-model="openingSide" :aria-label="t('binding.pair')">
          <SelectItem value="base">{{ pair.base }}</SelectItem>
          <SelectItem value="quote">{{ pair.quote }}</SelectItem>
        </Select>
        <span v-else-if="openingAsset" class="px-2 text-sm text-muted-foreground">{{ openingAsset.symbol.toUpperCase() }}</span>
      </template>
    </AmountInput>
  </FormField>

  <FormField v-if="!isSub" for="store-opening-at" :label="t('storeWizard.openingAt')">
    <DateTimePicker id="store-opening-at" v-model="openingAt" required :max="today" />
  </FormField>

  <SwitchRow v-if="!isSub" id="store-favorite" v-model="favorite" :label="t('storeWizard.favorite')" plain />
</template>
