<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { useSharedDue } from '@/entities/shared-event'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'

import type { SharedDueDialogProps } from '../shared-remainder.types'

/** `Due date`: the date the person promised to return the rest by; empty removes the term. */
const props = defineProps<SharedDueDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const mutation = useSharedDue()
const due = ref('')

watch(
  () => open.value,
  (now) => {
    if (now) due.value = props.person?.due_at ?? ''
  },
  { immediate: true },
)

function save() {
  if (!props.person) return
  mutation.mutate(
    {
      input: { event_id: props.event.event.id, participant_id: props.person.id, due_at: due.value || null },
      previous: props.person.due_at,
    },
    { onSuccess: () => (open.value = false) },
  )
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-due"
    form-id="shared-due-form"
    size="sm"
    :title="t('sharedEvents.due.title', { name: props.person?.name || t('sharedEvents.me') })"
    :description="t('sharedEvents.due.description')"
    :pending="mutation.isPending.value"
    phone-submit-only
    @submit="save"
  >
    <FormField for="shared-due-date" :label="t('sharedEvents.due.date')" :hint="t('sharedEvents.due.hint')">
      <DateTimePicker id="shared-due-date" v-model="due" mode="date" />
    </FormField>
  </FormDialog>
</template>
