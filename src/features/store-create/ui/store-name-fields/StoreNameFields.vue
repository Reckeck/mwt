<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { useContacts } from '@/shared/api/queries'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'

import { useStoreCreate } from '../../model/use-store-create-state'

/**
 * Step 2 of a top-level wallet (Wallets-1 · 3): its name, its currency (a bank's
 * first card, the cash), the bank behind it and whether it counts in the budget.
 */
const { t } = useI18n()
const { kind, name, nameTouched, withCard, cardCurrency, openingCurrency, contactId, offersBudget, inBudget, form } =
  useStoreCreate()
const contacts = useContacts(computed(() => kind.value === 'bank'))
const banks = computed(() => (contacts.data.value ?? []).filter((contact) => !contact.archived))
</script>

<template>
  <FormField for="store-name" :label="t('stores.name')" required :error="form.error('name')">
    <Input id="store-name" v-model="name" :placeholder="t(`storeWizard.namePlaceholder.${kind}`)" @input="nameTouched = true" />
  </FormField>

  <div class="grid gap-4 @md:grid-cols-2">
  <!-- A bank always has a card: the currency is the card's, it is created with the bank. -->
  <FormField
    v-if="withCard"
    for="store-card-currency"
    :label="t('storeWizard.currency')"
    required
    :error="form.error('cardCurrency')"
    :hint="t('storeWizard.currencyHint')"
    data-tour="store-bank-card"
  >
    <CurrencyPicker id="store-card-currency" v-model="cardCurrency" :kinds="['fiat']" label="" />
  </FormField>
  <FormField v-else-if="kind === 'cash'" for="store-cash-currency" :label="t('storeWizard.currency')" required :hint="t('storeWizard.currencyHint')">
    <CurrencyPicker id="store-cash-currency" v-model="openingCurrency" :kinds="['fiat']" label="" />
  </FormField>

  <FormField v-if="kind === 'bank'" for="store-contact" :label="t('storeWizard.bank')" :hint="t('storeWizard.bankHint')">
    <Select id="store-contact" v-model="contactId">
      <SelectItem value="">{{ t('stores.contactNone') }}</SelectItem>
      <SelectItem v-for="contact in banks" :key="contact.id" :value="contact.id">{{ contact.name }}</SelectItem>
    </Select>
  </FormField>
  </div>

  <SwitchRow
    v-if="offersBudget"
    id="store-in-budget"
    v-model="inBudget"
    :label="t('storeWizard.inBudget')"
    :hint="t('storeWizard.inBudgetHint')"
    plain
    data-tour="store-in-budget"
  />
</template>
