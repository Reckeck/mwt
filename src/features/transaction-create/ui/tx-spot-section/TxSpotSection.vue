<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'

import { useTxForm } from '../../model/use-tx-form'
import { AvailableHint } from '../available-hint'

/** Spot trade: sell one asset, buy another on an exchange. */
const { t } = useI18n()
const { form, check, props, noteExpression } = useTxForm()
</script>

<template>
  <div class="grid gap-3 @xl:grid-cols-2">
    <AssetInput v-model="form.soldAsset" required :error="check.error('soldAsset')" :label="t('form.sellAsset')" />
    <FormField for="tx-sold" :label="t('form.sellQty')" required :error="check.error('soldQty')">
      <AmountInput id="tx-sold" @expression="noteExpression('tx-sold', $event)" v-model="form.soldQty" />
      <AvailableHint :store-id="props.storeId" :asset="form.soldAsset" @max="(quantity) => (form.soldQty = quantity)" />
    </FormField>
    <AssetInput v-model="form.boughtAsset" required :error="check.error('boughtAsset')" :label="t('form.buyAsset')" />
    <FormField for="tx-bought" :label="t('form.buyQty')" required :error="check.error('boughtQty')">
      <AmountInput id="tx-bought" @expression="noteExpression('tx-bought', $event)" v-model="form.boughtQty" />
    </FormField>
  </div>
  <div class="grid gap-3 @xl:grid-cols-2">
    <div class="grid gap-1.5">
      <Label for="tx-liquidity">{{ t('tx.field.liquidity') }}</Label>
      <Select id="tx-liquidity" v-model="form.liquidity">
        <SelectItem value="">—</SelectItem>
        <SelectItem value="maker">{{ t('tx.liquidity.maker') }}</SelectItem>
        <SelectItem value="taker">{{ t('tx.liquidity.taker') }}</SelectItem>
      </Select>
    </div>
    <div class="grid gap-1.5">
      <Label for="tx-order">{{ t('tx.field.order') }}</Label>
      <Input id="tx-order" v-model="form.orderId" spellcheck="false" />
    </div>
  </div>
</template>
