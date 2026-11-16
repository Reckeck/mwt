<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { cryptoOperationDetailKey, shortAddress } from '@/entities/crypto-tx'
import { CategorySelect } from '@/features/category-select/@x/crypto-tx-override'
import { ContactPicker } from '@/features/contact-select/@x/crypto-tx-override'
import { call } from '@/shared/api/ipc'
import { useFormValidation } from '@/shared/lib/form'
import { Button } from '@/shared/ui/button'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import { buildOverride } from '../lib/build-override'
import { useSaveMeaning } from '../model/use-crypto-override'

/**
 * The meaning of an operation: its category (the same directory as every other operation), a description
 * and who it went through. The sum, the rate and the balance come from the network and are not editable.
 * «Apply to this contract» lays the category and the counterparty over every operation of the same contract.
 */
const props = defineProps<{ row: CryptoOpRow; focus?: 'category' | 'description' }>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const category = ref('')
const description = ref('')
const contact = ref('')
const wide = ref(false)

const GROUPS: CategoryGroup[] = ['income', 'fixed', 'daily', 'savings', 'service']

const detail = useQuery({
  queryKey: computed(() => cryptoOperationDetailKey(props.row.id, false)),
  queryFn: () => call<CryptoOperationDetail>('crypto_operation_detail', { input: { id: props.row.id, with_candidates: false } }),
  enabled: open,
  placeholderData: keepPreviousData,
})
const sameContract = computed(() => detail.data.value?.same_contract_count ?? 1)

watch(open, async (isOpen) => {
  if (!isOpen) return
  category.value = props.row.categories[0] ?? ''
  description.value = props.row.comment ?? ''
  contact.value = props.row.contact_id ?? ''
  wide.value = false
  await nextTick()
  document.getElementById(props.focus === 'description' ? 'crypto-meaning-description' : 'crypto-meaning-category')?.focus()
})

const form = useFormValidation(() => ({}), { resetOn: open })
const save = useSaveMeaning(() => {
  open.value = false
})
const counterparty = computed(() => (props.row.legs[0]?.direction === 'in' ? props.row.from : props.row.to))

function submit() {
  save.mutate(buildOverride(props.row, { category: category.value, description: description.value, contact: contact.value }, wide.value))
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    form-id="crypto-meaning-form"
    overlay="crypto-meaning"
    :title="t('cryptoTrack.operations.meaning.title')"
    :description="t('cryptoTrack.operations.meaning.hint')"
    :pending="save.isPending.value"
    phone-submit-only
    @submit="form.submit(submit, $event)"
  >
    <FormField :label="t('cryptoTrack.operations.meaning.category')" for="crypto-meaning-category" :hint="t('cryptoTrack.operations.meaning.categoryHint')">
      <CategorySelect v-model="category" id="crypto-meaning-category" :groups="GROUPS" no-create :placeholder="t('cryptoTrack.operations.meaning.categoryNone')" />
    </FormField>
    <FormField :label="t('cryptoTrack.operations.meaning.description')" for="crypto-meaning-description">
      <Input id="crypto-meaning-description" v-model="description" maxlength="1000" :placeholder="t('cryptoTrack.operations.meaning.descriptionHint')" />
    </FormField>
    <FormField
      :label="t('cryptoTrack.operations.meaning.through')"
      for="crypto-meaning-contact"
      :hint="counterparty ? t('cryptoTrack.operations.meaning.rememberHint', { address: shortAddress(counterparty) }) : undefined"
    >
      <ContactPicker v-model="contact" id="crypto-meaning-contact" :empty-label="t('cryptoTrack.operations.meaning.noContact')" no-create />
    </FormField>
    <p class="rounded-lg bg-muted p-3 text-sm text-muted-foreground">{{ t('cryptoTrack.operations.meaning.readonly') }}</p>
    <template v-if="sameContract > 1" #footer-start>
      <Button type="button" variant="outline" :class="wide && 'border-primary'" :aria-pressed="wide" data-crypto-contract @click="wide = !wide">
        {{ wide ? t('cryptoTrack.operations.meaning.contractOn', { count: sameContract }) : t('cryptoTrack.operations.meaning.contractAsk', { count: sameContract }) }}
      </Button>
    </template>
  </FormDialog>
</template>
