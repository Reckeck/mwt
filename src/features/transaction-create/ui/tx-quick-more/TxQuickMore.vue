<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AttachmentsField } from '@/entities/attachment'
import { StoreNodeSelect } from '@/entities/store'
import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { CHAIN_LABELS, CHAINS } from '@/shared/lib/chains'

import { useTxForm } from '../../model/use-tx-form'
import { AccountRefPicker } from '../account-ref-picker'
import { TxEventField } from '../tx-event-field'

/**
 * What the short form folds away (OpForm-1 · 5): date, fee, the other side,
 * the property, the network, files and links (Attach 5). The toggle names exactly what is inside, so
 * nothing hides behind a vague «Дополнительно».
 */
const { t } = useI18n()
const { form, check, showChain, lookupHint, properties, isProperty, editing, attachments, noteExpression, sharedEventOptions } =
  useTxForm()

const showSide = computed(() => form.kind === 'payment' || form.kind === 'topup')
const showProperty = computed(
  () => properties.value.length > 0 && !isProperty.value && ['topup', 'payment', 'transfer'].includes(form.kind),
)
/** Spot has its fee next to the price already. */
const showFee = computed(() => form.kind !== 'trade_spot')
const parts = computed(() =>
  [
    t('tx.more.date'),
    showFee.value && t('tx.more.fee'),
    showSide.value && t(form.kind === 'topup' ? 'tx.more.fromWhom' : 'tx.more.toWhom'),
    showProperty.value && t('tx.more.property'),
    sharedEventOptions.value.length > 0 && t('tx.more.event'),
    showChain.value && t('tx.more.network'),
    !editing.value && t('tx.more.files'),
  ]
    .filter(Boolean)
    .join(', '),
)
/** Opens by itself when a folded field already holds something or a save pointed at it. */
const hasValue = computed(() =>
  Boolean(
    form.timestamp ||
      (showFee.value && Number(form.feeAmount)) ||
      form.counterpart ||
      form.txHash ||
      attachments.value.length ||
      check.error('feeAmount'),
  ),
)
const feeSuffix = computed(() => {
  const asset = form.kind === 'convert' ? form.fromAsset : form.kind === 'trade_futures' ? form.settle : form.asset
  return asset?.symbol?.toUpperCase() ?? ''
})
</script>

<template>
  <AdvancedFields
    tone="primary"
    :label="t('tx.more.show', { fields: parts })"
    :open-label="t('tx.more.hide', { fields: parts })"
    :default-open="hasValue"
    data-tour="tx-more"
  >
    <div class="grid grid-cols-2 items-start gap-3">
      <FormField for="tx-date" :label="t('form.date')">
        <DateTimePicker id="tx-date" v-model="form.timestamp" mode="datetime" :placeholder="t('form.dateNow')" class="min-w-0 px-2" />
      </FormField>
      <FormField v-if="showFee" for="tx-fee" :label="t('form.fee')" :error="check.error('feeAmount')">
        <AmountInput id="tx-fee" @expression="noteExpression('tx-fee', $event)" v-model="form.feeAmount" placeholder="0">
          <template v-if="feeSuffix" #suffix>
            <span class="px-2 text-xs font-medium text-muted-foreground">{{ feeSuffix }}</span>
          </template>
        </AmountInput>
      </FormField>
      <FormField v-if="showSide" :label="form.kind === 'topup' ? t('tx.field.fromWhom') : t('tx.field.toWhom')">
        <AccountRefPicker v-model="form.counterpart" :stores="false" :empty-label="t('form.counterpartPlaceholder')" />
      </FormField>
      <FormField v-if="showProperty" for="tx-link-property" :label="t('tx.more.propertyLabel')">
        <StoreNodeSelect
          id="tx-link-property"
          v-model="form.propertyId"
          :nodes="properties"
          flat
          :empty-label="t('form.propertyPlaceholder')"
        />
      </FormField>
      <TxEventField />
    </div>
    <div v-if="showChain" class="grid items-start gap-3 @md:grid-cols-2">
      <FormField for="tx-network" :label="t('form.network')">
        <Select id="tx-network" v-model="form.network">
          <SelectItem value="">{{ t('form.noNetwork') }}</SelectItem>
          <SelectItem v-for="chain in CHAINS" :key="chain" :value="chain">{{ CHAIN_LABELS[chain] }}</SelectItem>
        </Select>
      </FormField>
      <FormField for="tx-hash" :label="t('form.txHash')" :hint="lookupHint || undefined">
        <Input id="tx-hash" v-model="form.txHash" spellcheck="false" placeholder="0x…" />
      </FormField>
    </div>
    <AttachmentsField v-if="!editing" v-model="attachments" />
  </AdvancedFields>
</template>
