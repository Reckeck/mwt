<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AssetChip } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { SwitchRow } from '@/shared/ui/switch-row'
import { accountStoreId } from '@/shared/lib/transactions'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'
import { AvailableHint } from '../available-hint'

/**
 * Transfer in the short form (OpForm-1 · 7): the sum, from where and to where.
 * A transfer to or from a person can be a debt — one switch; which way it goes
 * (lent, repaid, …) the app reads from the sides, the full editor names it.
 */
const { t } = useI18n()
const { form, check, debts, noteExpression } = useTxForm()
const fromStore = computed(() => accountStoreId(form.from))

const asDebt = computed({
  get: () => form.debt !== null,
  set: (on: boolean) => {
    form.debt = on ? (debts.value[0] ?? null) : null
  },
})
const amountError = computed(() => (check.error('amount') ? t('form.amountError') : check.error('asset')))
</script>

<template>
  <FormField for="tx-amount" :label="t('form.sum')" required :error="amountError">
    <AmountInput id="tx-amount" @expression="noteExpression('tx-amount', $event)" v-model="form.amount" size="lg" :placeholder="t('form.amountZero')">
      <template #suffix>
        <AssetChip v-model="form.asset" :label="t('form.asset')" :invalid="Boolean(check.error('asset'))" />
      </template>
    </AmountInput>
    <AvailableHint :store-id="fromStore" :asset="form.asset" @max="(quantity) => (form.amount = quantity)" />
  </FormField>

  <div class="grid items-start gap-3 @md:grid-cols-2">
    <FormField :label="t('tx.field.from')" required :error="check.error('from')">
      <AccountRefPicker v-model="form.from" />
    </FormField>
    <FormField :label="t('tx.field.to')" required :error="check.error('to')">
      <AccountRefPicker v-model="form.to" :exclude-store-id="fromStore" />
    </FormField>
  </div>

  <SwitchRow v-if="debts.length" id="tx-debt" v-model="asDebt" :label="t('tx.quick.asDebt')" plain class="justify-start flex-row-reverse" />
</template>
