<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { TxCategorySelect } from '../tx-category-select'
import { ContactPicker } from '@/entities/contact'
import { Money } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'
import { CHAIN_LABELS, CHAINS } from '@/shared/lib/chains'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import { TxEventField } from '../tx-event-field'
import { categoryGroupsFor } from '../../transaction-form.model'
import { useTxForm } from '../../model/use-tx-form'

/**
 * What every kind shares, under its own fields: fee and date, categories
 * (with the cashback source), comment; network, hash and contact sit folded
 * under «Дополнительно».
 */
const { t } = useI18n()
const { form, check, props, showChain, lookupHint, rateHint, needsCategory, cashbackSources, properties, isProperty, noteExpression } =
  useTxForm()

// The network fills in by itself from the asset, so only what the user set opens the section.
/** Rent, fuel, repair: the operation can belong to a property (plan G4); a property's own buy or sell cannot. */
const showProperty = computed(
  () => properties.value.length > 0 && !isProperty.value && ['topup', 'payment', 'transfer'].includes(form.kind),
)
const hasAdvanced = computed(() => Boolean(form.txHash || form.contactId))
</script>

<template>
  <!-- No price field: the backend prices the operation at the rates of its moment
       (a price from an exchange fill or the calculator still travels in `form.price`). -->
  <div class="grid items-start gap-3 @xl:grid-cols-2">
    <div class="grid gap-1.5">
      <Label for="tx-fee">{{ t('form.fee') }}</Label>
      <AmountInput id="tx-fee" @expression="noteExpression('tx-fee', $event)" v-model="form.feeAmount" placeholder="0" />
    </div>
    <div v-if="!props.template" class="grid min-w-0 gap-1.5">
      <Label for="tx-date">{{ t('form.date') }}</Label>
      <DateTimePicker id="tx-date" v-model="form.timestamp" mode="datetime" :placeholder="t('form.dateNow')" class="min-w-0 px-2" />
      <p v-if="rateHint && form.kind !== 'asset_sell' && form.kind !== 'trade_futures'" class="text-xs text-muted-foreground">
        <template v-if="rateHint.text">{{ rateHint.text }}</template>
        <i18n-t v-else keypath="tx.rate.at" tag="span">
          <template #date>{{ rateHint.date }}</template>
          <template #price>
            1 {{ rateHint.symbol }} = <Money :amount="rateHint.amount" :currency="rateHint.currency" :decimals="rateHint.decimals" />
          </template>
        </i18n-t>
      </p>
    </div>
  </div>

  <FormField
    for="tx-categories"
    :label="t('form.categories')"
    :required="needsCategory && !props.template"
    :error="check.error('categories')"
    :hint="needsCategory ? t('tx.field.categoryHint') : undefined"
  >
    <TxCategorySelect id="tx-categories" v-model="form.categories" multiple :groups="categoryGroupsFor(form.kind)" />
  </FormField>

  <!-- Cashback: which card's purchases earned it, for the cashback-by-card report. -->
  <div v-if="form.kind === 'topup' && form.categories.includes('cashback')" class="grid gap-1.5">
    <Label for="tx-cashback-source">{{ t('loyalty.source') }}</Label>
    <StoreNodeSelect
      id="tx-cashback-source"
      v-model="form.sourceStoreId"
      :nodes="cashbackSources"
      flat
      :empty-label="t('loyalty.noSource')"
    />
  </div>

  <div v-if="showProperty" class="grid gap-1.5">
    <Label for="tx-link-property">{{ t('tx.field.linkProperty') }}</Label>
    <StoreNodeSelect
      id="tx-link-property"
      v-model="form.propertyId"
      :nodes="properties"
      flat
      :empty-label="t('tx.field.noLinkProperty')"
    />
    <p class="text-xs text-muted-foreground">{{ t('tx.field.linkPropertyHint') }}</p>
  </div>

  <TxEventField />

  <div class="grid gap-1.5">
    <Label for="tx-comment">{{ t('form.comment') }}</Label>
    <Textarea id="tx-comment" v-model="form.comment" rows="1" :placeholder="t('form.commentHint')" />
  </div>

  <!-- Rarely needed: open by itself when one of these already has a value (editing, a template). -->
  <AdvancedFields :default-open="hasAdvanced">
    <div v-if="showChain" class="grid gap-3 @xl:grid-cols-2">
      <div class="grid gap-1.5">
        <Label for="tx-network">{{ t('form.network') }}</Label>
        <Select id="tx-network" v-model="form.network">
          <SelectItem value="">{{ t('form.noNetwork') }}</SelectItem>
          <SelectItem v-for="chain in CHAINS" :key="chain" :value="chain">{{ CHAIN_LABELS[chain] }}</SelectItem>
        </Select>
      </div>
      <div class="grid gap-1.5">
        <Label for="tx-hash">{{ t('form.txHash') }}</Label>
        <Input id="tx-hash" v-model="form.txHash" spellcheck="false" placeholder="0x…" />
        <p v-if="lookupHint" class="text-xs text-muted-foreground">{{ lookupHint }}</p>
      </div>
    </div>
    <div class="grid gap-1.5">
      <Label for="tx-contact">{{ t('tx.field.contact') }}</Label>
      <ContactPicker id="tx-contact" v-model="form.contactId" :empty-label="t('tx.side.unspecified')" />
      <p class="text-xs text-muted-foreground">{{ t('contactPicker.hint') }}</p>
    </div>
  </AdvancedFields>
</template>
