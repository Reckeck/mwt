<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Tag } from 'lucide-vue-next'

import { CurrencyPicker } from '@/entities/currency'
import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { FormDialog } from '@/shared/ui/form-dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'

import type { AssetRevalueButtonProps } from './asset-revalue.types'
import { thingKey, usePropertyValues, useValuationMutations } from './model/use-valuations'
import { ValuationList } from './ui/valuation-list'

const props = defineProps<AssetRevalueButtonProps>()

const { t } = useI18n()
const settings = useSettingsStore()

const open = ref(false)
const date = ref('')
const price = ref('')
const currency = ref('')

const values = usePropertyValues(() => props.storeId)
const value = computed(() => values.data.value?.find((row) => row.asset_key === thingKey(props.asset)) ?? null)
const base = computed(() => {
  const row = value.value
  if (!row?.base || !row.base_date || !row.base_price) return null
  const currency = row.base === 'purchase' ? (row.purchase?.currency ?? row.currency) : row.currency
  return { kind: row.base, date: row.base_date, price: row.base_price, currency }
})

const form = useFormValidation(
  () => ({
    date: required(date.value, 'pick'),
    price: positiveAmount(price.value),
    currency: required(currency.value, 'pick'),
  }),
  { resetOn: open },
)

watch(open, (isOpen) => {
  if (!isOpen) return
  date.value = new Date().toISOString().slice(0, 10)
  price.value = ''
  currency.value = value.value?.currency || settings.baseCurrency
})

const { add, remove, reset } = useValuationMutations(() => {
  price.value = ''
})

const save = () =>
  add.mutate({
    store_id: props.storeId,
    asset_key: thingKey(props.asset),
    date: date.value,
    unit_price: price.value.trim().replace(',', '.'),
    currency: currency.value,
  })
</script>

<template>
  <Button variant="ghost" size="sm" @click="open = true">
    <Tag class="size-4" />
    {{ t('asset.revalue') }}
  </Button>

  <FormDialog overlay="asset-revalue"
    v-model:open="open"
    :title="t('asset.revalueTitle', { name: props.asset.name || props.asset.symbol })"
    form-id="asset-revalue-form"
    :submit-label="t('valuation.add')"
    :pending="add.isPending.value"
    @submit="form.submit(save, $event)"
  >
      <div class="grid gap-3 @mid:grid-cols-[9rem_1fr_8rem]">
        <FormField for="revalue-date" :label="t('valuation.date')" required :error="form.error('date')">
          <DateTimePicker id="revalue-date" v-model="date" />
        </FormField>
        <FormField for="revalue-price" :label="t('tx.field.unitPrice')" required :error="form.error('price')">
          <AmountInput id="revalue-price" v-model="price" />
        </FormField>
        <FormField for="revalue-currency" :label="t('tx.field.currency')" required :error="form.error('currency')">
          <CurrencyPicker id="revalue-currency" v-model="currency" />
        </FormField>
      </div>
      <p class="text-xs text-muted-foreground">{{ t('valuation.hint') }}</p>
      <ValuationList
        v-if="value && (value.valuations.length || base)"
        :valuations="value.valuations"
        :base="base"
        :pending="remove.isPending.value"
        @remove="remove.mutate"
      />
    <template v-if="value?.base === 'manual'" #footer-start>
      <Button variant="ghost" type="button" :disabled="reset.isPending.value" @click="reset.mutate(value.asset_key)">
        {{ t('asset.revalueReset') }}
      </Button>
    </template>
  </FormDialog>
</template>
