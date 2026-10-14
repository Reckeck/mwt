<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'

import { useTxForm } from '../../model/use-tx-form'

/** Futures: open or close a position on a contract. */
const { t } = useI18n()
const { form, check } = useTxForm()
</script>

<template>
  <div class="grid gap-3 @sm:grid-cols-3">
    <FormField for="tx-symbol" :label="t('tx.field.contract')" required :error="check.error('symbol')">
      <Input id="tx-symbol" v-model="form.symbol" placeholder="BTCUSDT" spellcheck="false" />
    </FormField>
    <div class="grid gap-1.5">
      <Label for="tx-direction">{{ t('tx.field.direction') }}</Label>
      <Select id="tx-direction" v-model="form.direction">
        <option value="long">{{ t('tx.direction.long') }}</option>
        <option value="short">{{ t('tx.direction.short') }}</option>
      </Select>
    </div>
    <div class="grid gap-1.5">
      <Label for="tx-futures-action">{{ t('tx.field.action') }}</Label>
      <Select id="tx-futures-action" v-model="form.action">
        <option value="open">{{ t('tx.futures.open') }}</option>
        <option value="close">{{ t('tx.futures.close') }}</option>
      </Select>
    </div>
    <FormField for="tx-qty" :label="t('tx.field.quantity')" required :error="check.error('quantity')">
      <AmountInput id="tx-qty" v-model="form.quantity" />
    </FormField>
    <FormField for="tx-fill" :label="t('tx.field.price')" required :error="check.error('fillPrice')">
      <AmountInput id="tx-fill" v-model="form.fillPrice" />
    </FormField>
    <div class="grid gap-1.5">
      <Label for="tx-leverage">{{ t('tx.field.leverage') }}</Label>
      <AmountInput id="tx-leverage" v-model="form.leverage" />
    </div>
    <AssetInput v-model="form.settle" :label="t('tx.field.settle')" />
    <div class="grid gap-1.5">
      <Label for="tx-futures-category">{{ t('tx.field.futuresCategory') }}</Label>
      <Select id="tx-futures-category" v-model="form.futuresCategory">
        <option value="linear">{{ t('tx.futuresCategory.linear') }}</option>
        <option value="inverse">{{ t('tx.futuresCategory.inverse') }}</option>
      </Select>
    </div>
    <div v-if="form.action === 'close'" class="grid gap-1.5">
      <Label for="tx-pnl">{{ t('tx.field.closedPnl') }}</Label>
      <AmountInput id="tx-pnl" v-model="form.closedPnl" />
    </div>
  </div>
</template>
