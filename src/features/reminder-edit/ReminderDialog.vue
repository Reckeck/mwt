<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call, parseError } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { toIsoTimestamp, toLocalInput } from '@/shared/lib/datetime'

import type { ReminderDialogProps } from './reminder-edit.types'

const props = withDefaults(defineProps<ReminderDialogProps>(), { reminder: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const text = ref('')
const dueAt = ref('')

watch(open, (isOpen) => {
  if (!isOpen) return
  text.value = props.reminder?.text ?? ''
  const tomorrow = new Date(Date.now() + 86_400_000)
  tomorrow.setHours(9, 0, 0, 0)
  dueAt.value = toLocalInput(props.reminder?.due_at ?? tomorrow.toISOString())
})

const valid = computed(() => text.value.trim() !== '' && toIsoTimestamp(dueAt.value) !== null)

const save = useMutation({
  mutationFn: () => {
    const input: ReminderInput = { text: text.value.trim(), due_at: toIsoTimestamp(dueAt.value) ?? '' }
    return props.reminder
      ? call<Reminder>('update_reminder', { input: { id: props.reminder.id, reminder: input } })
      : call<Reminder>('create_reminder', { input })
  },
  onSuccess: () => {
    toast.success(props.reminder ? t('reminder.saved') : t('reminder.created'))
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  onError: (err) => toast.error(t('reminder.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.reminder ? t('reminder.edit') : t('reminder.new')">
    <form id="reminder-form" class="grid gap-4" @submit.prevent="valid && save.mutate()">
      <div class="grid gap-1.5">
        <Label for="reminder-text">{{ t('reminder.text') }}</Label>
        <Input id="reminder-text" v-model="text" />
      </div>
      <div class="grid gap-1.5">
        <Label for="reminder-due">{{ t('reminder.dueAt') }}</Label>
        <Input id="reminder-due" v-model="dueAt" type="datetime-local" />
      </div>
    </form>
    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="reminder-form" type="submit" :disabled="!valid || save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
