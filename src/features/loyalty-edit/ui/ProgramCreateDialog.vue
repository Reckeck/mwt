<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { useContacts } from '@/shared/api/queries'

import { invalidateLoyalty } from '../model/invalidate'
import { useLoyaltyInfo } from '../model/use-loyalty-info'
import LoyaltyInfoFields from './LoyaltyInfoFields.vue'
import { PROGRAM_COLOR } from '../loyalty-edit.constants'

/** «Добавить программу»: the programme and its points in one step (`create_loyalty_program`). */
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ created: [store: Store] }>()
const { t } = useI18n()
const queryClient = useQueryClient()

const name = ref('')
const contactId = ref('')
const state = useLoyaltyInfo()
const contacts = useContacts(open)

const form = useFormValidation(() => ({ name: required(name.value), ...state.rules() }), { resetOn: open })
watch(open, (value) => {
  if (!value) return
  name.value = ''
  contactId.value = ''
  state.load(null)
})

const save = useMutation({
  mutationFn: () =>
    call<Store>('create_loyalty_program', {
      input: {
        name: name.value.trim(),
        color: PROGRAM_COLOR,
        contact_id: contactId.value || null,
        info: state.toInfo(),
      } satisfies CreateLoyaltyProgramInput,
    }),
  onSuccess: (store) => {
    invalidateLoyalty(queryClient)
    open.value = false
    emit('created', store)
  },
  meta: { error: 'bonuses.createFailed', success: 'bonuses.created' },
})
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="t('bonuses.add')"
    :description="t('bonuses.addHint')"
    size="lg"
    form-id="program-create-form"
    :submit-label="t('bonuses.add')"
    :pending="save.isPending.value"
    @submit="form.submit(save.mutate, $event)"
  >
    <div class="grid gap-4 sm:grid-cols-2">
      <FormField for="program-name" :label="t('bonuses.name')" required :error="form.error('name')">
        <Input id="program-name" v-model="name" maxlength="64" :placeholder="t('bonuses.namePlaceholder')" />
      </FormField>
      <div class="grid gap-1.5">
        <Label for="program-contact">{{ t('stores.contact') }}</Label>
        <Select id="program-contact" v-model="contactId">
          <SelectItem value="">{{ t('stores.contactNone') }}</SelectItem>
          <SelectItem
            v-for="contact in (contacts.data.value ?? []).filter((entry) => !entry.archived)"
            :key="contact.id"
            :value="contact.id"
          >
            {{ contact.name }}
          </SelectItem>
        </Select>
      </div>
    </div>
    <LoyaltyInfoFields :state="state" :error="form.error" />
  </FormDialog>
</template>
