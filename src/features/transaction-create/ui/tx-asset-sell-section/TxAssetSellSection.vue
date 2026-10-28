<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AssetInput, AssetSelect } from '@/entities/asset'
import { StoreNodeSelect } from '@/entities/store'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'

import { useTxForm } from '../../model/use-tx-form'

/** Selling property: what, how many units, at what price, and where the money goes. */
const { t } = useI18n()
const { form, check, heldAssets, proceedsStores, noteExpression } = useTxForm()
</script>

<template>
  <div class="grid gap-3 @xl:grid-cols-2">
    <FormField for="tx-sell-asset" :label="t('tx.field.property')" required :error="check.error('asset')">
      <AssetSelect
        id="tx-sell-asset"
        v-model="form.asset"
        :assets="heldAssets"
        :placeholder="t('tx.field.pickProperty')"
        :invalid="Boolean(check.error('asset'))"
      />
    </FormField>
    <FormField for="tx-sell-qty" :label="t('tx.field.units')" required :error="check.error('sellQty')">
      <AmountInput id="tx-sell-qty" @expression="noteExpression('tx-sell-qty', $event)" v-model="form.sellQty" />
    </FormField>
    <FormField for="tx-sell-price" :label="t('tx.field.unitPrice')" required :error="check.error('sellPrice')">
      <AmountInput id="tx-sell-price" @expression="noteExpression('tx-sell-price', $event)" v-model="form.sellPrice" />
    </FormField>
    <AssetInput v-model="form.currency" required :error="check.error('currency')" :label="t('tx.field.currency')" />
  </div>
  <div class="grid gap-1.5">
    <Label for="tx-proceeds">{{ t('tx.field.proceedsTo') }}</Label>
    <StoreNodeSelect id="tx-proceeds" v-model="form.proceedsTo" :nodes="proceedsStores" flat :empty-label="t('tx.side.unspecified')" />
  </div>
</template>
