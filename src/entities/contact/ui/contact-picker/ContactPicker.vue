<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Combobox, type ComboboxOption } from '@/shared/ui/combobox'
import { useContacts } from '@/shared/api/queries'
import { useContactCreator } from '@/shared/lib/contact-create'

import type { ContactPickerProps } from './contact-picker.types'

/**
 * Who an operation, a wallet or a bonus programme is with: a searchable list of
 * contacts. Nothing found for `Pete` offers `Create "Pete"`: the full contact
 * form opens over the screen with the name filled in (the shell hosts it, see
 * `useContactCreator`), and the new contact is picked when it is saved — what was
 * typed in the screen below stays as it was.
 */
const props = defineProps<ContactPickerProps>()
const emit = defineEmits<{ guest: [name: string] }>()

/** Contact id; `''` is «no contact». */
const model = defineModel<string>({ required: true })

const { t } = useI18n()
const contacts = useContacts()

/** The «no contact» row: Reka refuses an item whose value is `''`. */
const NONE = 'none'

const options = computed<ComboboxOption[]>(() => {
  const rows = (contacts.data.value ?? [])
    .filter((contact) => !contact.archived || contact.id === model.value)
    .sort((left, right) => left.name.localeCompare(right.name))
    .map((contact) => ({ value: contact.id, label: contact.name }))
  return props.emptyLabel ? [{ value: NONE, label: props.emptyLabel }, ...rows] : rows
})

const choice = computed({
  get: () => model.value || (props.emptyLabel ? NONE : ''),
  set: (value: string | string[]) => {
    const next = Array.isArray(value) ? (value[0] ?? '') : value
    model.value = next === NONE ? '' : next
  },
})

const creator = useContactCreator()
/** Opens the shell's contact form with the typed name; the saved contact is picked. */
async function startCreate(name: string) {
  const contact = await creator?.(name)
  if (contact) model.value = contact.id
}

const createLabel = computed(() =>
  props.noCreate || !creator ? undefined : (text: string) => t('contactPicker.create', { name: text }),
)

const guestLabel = computed(() =>
  props.guestLabel ? (text: string) => t('contactPicker.guest', { name: text }) : undefined,
)
</script>

<template>
  <Combobox
    v-model="choice"
    :id="props.id"
    :options="options"
    :placeholder="props.placeholder ?? t('contactPicker.placeholder')"
    :aria-label="props.ariaLabel"
    :empty-label="t('contactPicker.empty')"
    :create-label="createLabel"
    :disabled="props.disabled"
    :guest-label="guestLabel"
    @create="startCreate"
    @guest="(name) => emit('guest', name)"
  />
</template>
