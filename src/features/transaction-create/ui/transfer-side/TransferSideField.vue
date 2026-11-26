<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'

import { AccountRefPicker } from '../account-ref-picker'

/**
 * One side of a transfer or an exchange: a wallet, or (`canContact`, a transfer only) a contact
 * instead, which makes it a loan out or in. Picking the one clears the other. The contact
 * picker comes in the `contact` slot (the owner of the form knows the contact list).
 */
const props = defineProps<{ id: string; label: string; error?: string; canContact: boolean }>()
const wallet = defineModel<string>('wallet', { default: '' })
const contact = defineModel<string>('contact', { default: '' })
const { t } = useI18n()

const byContact = ref(Boolean(contact.value))
watch(contact, (id) => {
  if (id) byContact.value = true
})
watch(
  () => props.canContact,
  (allowed) => {
    if (!allowed && !contact.value) byContact.value = false
  },
)

function toggle() {
  byContact.value = !byContact.value
  if (byContact.value) wallet.value = ''
  else contact.value = ''
}
</script>

<template>
  <FormField :for="props.id" :label="props.label" required :error="props.error">
    <slot v-if="byContact" name="contact" :id="props.id" :invalid="Boolean(props.error)" />
    <AccountRefPicker v-else :id="props.id" v-model="wallet" variant="plate" :invalid="Boolean(props.error)" />
    <Button v-if="byContact || props.canContact" type="button" variant="link" size="sm" class="h-auto justify-self-start px-0" data-transfer-side-toggle @click="toggle">
      {{ byContact ? t('quickAdd.sideWallet') : t('quickAdd.sideContact') }}
    </Button>
  </FormField>
</template>
