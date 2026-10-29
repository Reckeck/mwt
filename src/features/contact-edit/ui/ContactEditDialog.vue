<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Input } from '@/shared/ui/input'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'
import { call } from '@/shared/api/ipc'
import { useContacts } from '@/shared/api/queries'
import { required, useFormValidation } from '@/shared/lib/form'

import { CONTACT_KINDS } from '../contact-edit.constants'
import type { ContactEditDialogProps } from '../contact-edit.types'
import { draftOf } from '../model/use-contact-save'
import ContactArchive from './ContactArchive.vue'

/** `+ Contact` and `⋯ › Edit`: name, kind, group and note in one form; requisites live in the card. */
const props = defineProps<ContactEditDialogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [contact: Contact]; deleted: [] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const name = ref('')
const kind = ref<ContactKind>('person')
const group = ref('')
const note = ref('')
watch(open, (now) => {
  if (!now) return
  name.value = props.contact?.name ?? props.initialName?.trim() ?? ''
  kind.value = props.contact?.kind ?? 'person'
  group.value = props.contact?.group ?? ''
  note.value = props.contact?.note ?? ''
})
const form = useFormValidation(() => ({ name: required(name.value) }), { resetOn: open })

/** Groups already in use, offered as the user types. */
const contacts = useContacts()
const groups = computed(() =>
  [...new Set((contacts.data.value ?? []).map((entry) => entry.group.trim()).filter(Boolean))].sort((left, right) =>
    left.localeCompare(right),
  ),
)

const save = useMutation({
  mutationFn: () => {
    const fields = { name: name.value.trim(), kind: kind.value, group: group.value.trim(), note: note.value.trim() }
    return props.contact
      ? call<Contact>('update_contact', { input: { id: props.contact.id, contact: draftOf(props.contact, fields) } })
      : call<Contact>('create_contact', { input: { ...fields, accounts: [] } })
  },
  onSuccess: (contact) => {
    void queryClient.invalidateQueries({ queryKey: ['contacts'] })
    open.value = false
    emit('saved', contact)
  },
  meta: {
    success: (data) => (props.contact ? t('contacts.saved') : t('contacts.createdNamed', { name: (data as Contact).name })),
    error: 'contacts.saveFailed',
  },
})
function onDeleted() {
  open.value = false
  emit('deleted')
}
</script>

<template>
  <FormDialog overlay="contact-edit"
    v-model:open="open"
    :title="props.contact ? t('contacts.edit') : t('contacts.new')"
    size="sm"
    form-id="contact-edit"
    :submit-label="props.contact ? t('actions.save') : t('contacts.create')"
    :pending="save.isPending.value"
    @submit="form.submit(save.mutate, $event)"
  >
    <FormField for="contact-edit-name" :label="t('contacts.name')" required :error="form.error('name')">
      <Input id="contact-edit-name" v-model="name" autocomplete="off" />
    </FormField>
    <div class="grid gap-4 @sm:grid-cols-2">
      <FormField :label="t('contacts.kind')">
        <Select v-model="kind" :aria-label="t('contacts.kind')">
          <SelectItem v-for="entry in CONTACT_KINDS" :key="entry" :value="entry">{{ t(`contactKind.${entry}`) }}</SelectItem>
        </Select>
      </FormField>
      <FormField for="contact-edit-group" :label="t('contacts.group')">
        <Input id="contact-edit-group" v-model="group" list="contact-edit-groups" autocomplete="off" />
        <datalist id="contact-edit-groups">
          <option v-for="entry in groups" :key="entry" :value="entry" />
        </datalist>
      </FormField>
    </div>
    <FormField for="contact-edit-note" :label="t('stores.note')">
      <Textarea id="contact-edit-note" v-model="note" rows="2" :placeholder="t('contacts.notePlaceholder')" />
    </FormField>

    <p v-if="!props.contact" class="text-xs text-muted-foreground">{{ t('contacts.requisitesLater') }}</p>

    <template v-if="props.contact" #footer-start>
      <ContactArchive :contact="props.contact" @deleted="onDeleted" />
    </template>
  </FormDialog>
</template>
