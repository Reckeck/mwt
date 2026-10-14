<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'
import { required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { cn } from '@/shared/lib/utils'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'

import { updateInputOf, useStoreUpdate } from '../model/use-store-update'
import type { StoreFormProps } from '../store-edit.types'

const props = defineProps<StoreFormProps>()
const { t } = useI18n()

const PURPOSES: Purpose[] = ['everyday', 'savings', 'investment', 'transit']

const name = ref('')
const note = ref('')
const color = ref('')
const purpose = ref<Purpose>('everyday')
const contactId = ref('')

const form = useFormValidation(() => ({ name: required(name.value) }), { resetOn: () => props.store.id })

/** Only a top-level wallet belongs to a contact (its sub-accounts share it). */
const isRoot = computed(() => !props.store.parent_id)
/** A loan's purpose is fixed: it is never everyday money. */
const purposeEditable = computed(() => props.store.kind !== 'liability')

const contacts = useQuery({
  queryKey: ['contacts'],
  queryFn: () => call<Contact[]>('list_contacts'),
  enabled: isRoot,
})

watch(
  () => props.store,
  (store) => {
    name.value = store.name
    note.value = store.note
    color.value = store.color
    purpose.value = store.purpose
    contactId.value = store.contact_id ?? ''
  },
  { immediate: true },
)

const save = useStoreUpdate()

function submit() {
  save.mutate(
    updateInputOf(props.store, {
      name: name.value.trim(),
      color: color.value,
      note: note.value.trim(),
      purpose: purposeEditable.value ? purpose.value : null,
      contact_id: isRoot.value ? contactId.value || null : props.store.contact_id,
    }),
  )
}
</script>

<template>
  <form class="@container grid gap-5" @submit.prevent="form.submit(submit, $event)">
    <div class="grid grid-cols-[1fr_auto] gap-4">
      <FormField for="store-name" :label="t('stores.name')" required :error="form.error('name')">
        <Input id="store-name" v-model="name" />
      </FormField>
      <div class="grid gap-1.5">
        <Label for="store-color">{{ t('stores.color') }}</Label>
        <input
          id="store-color"
          v-model="color"
          type="color"
          class="h-9 w-14 cursor-pointer rounded-md border border-input bg-transparent p-1"
        >
      </div>
    </div>

    <fieldset v-if="purposeEditable" class="grid gap-2" data-tour="wallet-purpose">
      <legend class="mb-1 text-sm font-medium">{{ t('purpose.title') }}</legend>
      <RadioGroup v-model="purpose" class="grid gap-2 @md:grid-cols-2">
        <label
          v-for="entry in PURPOSES"
          :key="entry"
          :for="`store-purpose-${entry}`"
          :class="
            cn(
              'flex cursor-pointer items-start gap-3 rounded-lg border border-border p-3 transition-colors hover:bg-accent has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring',
              purpose === entry && 'border-primary bg-primary/5',
            )
          "
        >
          <RadioGroupItem :id="`store-purpose-${entry}`" :value="entry" class="mt-0.5" />
          <span class="grid gap-0.5">
            <span class="text-sm font-medium">{{ t(`purpose.${entry}`) }}</span>
            <span class="text-xs text-muted-foreground">{{ t(`purpose.${entry}Hint`) }}</span>
            <span class="text-xs italic text-muted-foreground">{{ t(`purpose.${entry}Example`) }}</span>
          </span>
        </label>
      </RadioGroup>
    </fieldset>

    <div v-if="isRoot" class="grid gap-1.5">
      <Label for="store-contact">{{ t('stores.contact') }}</Label>
      <Select id="store-contact" v-model="contactId">
        <option value="">{{ t('stores.contactNone') }}</option>
        <option
          v-for="contact in (contacts.data.value ?? []).filter((entry) => !entry.archived)"
          :key="contact.id"
          :value="contact.id"
        >
          {{ contact.name }}
        </option>
      </Select>
    </div>

    <div class="grid gap-1.5">
      <Label for="store-note">{{ t('stores.note') }}</Label>
      <Textarea id="store-note" v-model="note" rows="2" />
    </div>

    <div>
      <Button type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
