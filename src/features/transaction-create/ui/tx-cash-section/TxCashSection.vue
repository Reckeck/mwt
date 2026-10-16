<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'
import { SplitRows } from '../split-rows'

/** Top-up, payment and DeFi: one asset on this wallet. */
const { t } = useI18n()
const { form, check, props, isProperty, propertyName, overrunText } = useTxForm()
</script>

<template>
  <div class="grid gap-3 @sm:grid-cols-2">
    <div v-if="isProperty && form.kind === 'topup'" class="grid gap-1.5">
      <Label for="tx-property">{{ t('tx.field.property') }}</Label>
      <Input id="tx-property" v-model="propertyName" :placeholder="t('tx.field.propertyHint')" />
    </div>
    <AssetInput v-else v-model="form.asset" required :error="check.error('asset')" :label="t('form.asset')" />
    <FormField for="tx-amount" :label="isProperty ? t('tx.field.units') : t('form.amount')" required :error="check.error('amount')">
      <AmountInput id="tx-amount" v-model="form.amount" :placeholder="t('form.amountHint')" />
    </FormField>
  </div>
  <p v-if="overrunText" class="rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-xs text-warning">
    {{ overrunText }}
  </p>
  <div v-if="isProperty && form.kind === 'topup'" class="grid gap-1.5">
    <Label for="tx-unit-price">{{ t('tx.field.unitPrice') }}</Label>
    <AmountInput id="tx-unit-price" v-model="form.unitPrice" />
    <p class="text-xs text-muted-foreground">{{ t('tx.field.unitPriceHint') }}</p>
  </div>
  <div v-if="form.kind === 'defi'" class="grid gap-3 @sm:grid-cols-3">
    <div class="grid gap-1.5">
      <Label for="tx-defi-action">{{ t('tx.field.action') }}</Label>
      <Select id="tx-defi-action" v-model="form.defiAction">
        <SelectItem v-for="entry in ['stake', 'unstake', 'claim', 'add_liquidity', 'remove_liquidity']" :key="entry" :value="entry">
          {{ t(`tx.defiAction.${entry}`) }}
        </SelectItem>
      </Select>
    </div>
    <div class="grid gap-1.5">
      <Label for="tx-protocol">{{ t('tx.field.protocol') }}</Label>
      <Input id="tx-protocol" v-model="form.protocol" />
    </div>
    <div class="grid gap-1.5">
      <Label for="tx-pool">{{ t('tx.field.pool') }}</Label>
      <Input id="tx-pool" v-model="form.poolId" />
    </div>
  </div>
  <div v-if="form.kind !== 'defi'" class="grid gap-1.5">
    <Label>{{ form.kind === 'topup' ? t('tx.field.fromWhom') : t('tx.field.toWhom') }}</Label>
    <AccountRefPicker v-model="form.counterpart" :stores="false" :empty-label="t('tx.side.unspecified')" />
  </div>
  <div v-if="form.kind === 'topup' && !props.template && !isProperty" class="grid gap-1.5">
    <p v-if="form.extra.length" class="text-xs text-muted-foreground">{{ t('tx.split.topupHint') }}</p>
    <FormField :error="check.error('extra')">
      <SplitRows v-model="form.extra" kind="topup" mode="direct" />
    </FormField>
  </div>
</template>
