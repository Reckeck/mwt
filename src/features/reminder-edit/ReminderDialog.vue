<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS, localDate } from '@/shared/lib/planning'

import { REPEAT_UNITS } from './reminder-edit.constants'
import type { ReminderDialogProps } from './reminder-edit.types'

const props = withDefaults(defineProps<ReminderDialogProps>(), { reminder: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const text = ref('')
/** `YYYY-MM-DD` and `HH:MM`, local: two fields read better than one date-time picker. */
const date = ref('')
const time = ref('09:00')
/** `''` = once. */
const unit = ref<RepeatUnit | ''>('')
const every = ref('1')

const pad = (value: number) => String(value).padStart(2, '0')

watch(open, (isOpen) => {
  if (!isOpen) return
  const reminder = props.reminder
  text.value = reminder?.text ?? ''
  const due = reminder ? new Date(reminder.due_at) : new Date(Date.now() + 86_400_000)
  date.value = localDate(due)
  time.value = reminder ? `${pad(due.getHours())}:${pad(due.getMinutes())}` : '09:00'
  unit.value = reminder?.repeat?.unit ?? ''
  every.value = String(reminder?.repeat?.every ?? 1)
})

const dueAt = computed(() => {
  if (!date.value || !time.value) return null
  const parsed = new Date(`${date.value}T${time.value}`)
  return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString()
})
const repeat = computed<ReminderRepeat | null>(() =>
  unit.value ? { unit: unit.value, every: Math.max(1, Math.round(Number(every.value) || 1)) } : null,
)

const form = useFormValidation(
  () => ({
    text: required(text.value),
    date: required(date.value) ?? (dueAt.value ? null : t('validation.date')),
    time: required(time.value),
  }),
  { resetOn: open },
)

const save = useMutation({
  mutationFn: () => {
    const input: ReminderInput = { text: text.value.trim(), due_at: dueAt.value ?? '', repeat: repeat.value }
    return props.reminder
      ? call<Reminder>('update_reminder', { input: { id: props.reminder.id, reminder: input } })
      : call<Reminder>('create_reminder', { input })
  },
  onSuccess: () => {
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'reminder.saveFailed', success: () => props.reminder ? t('reminder.saved') : t('reminder.created') },
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.reminder ? t('reminder.edit') : t('reminder.new')">
    <form id="reminder-form" class="grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <FormField for="reminder-text" :label="t('reminder.text')" required :error="form.error('text')">
        <Input id="reminder-text" v-model="text" />
      </FormField>
      <div class="grid grid-cols-[1fr_8rem] gap-3">
        <FormField for="reminder-date" :label="t('reminder.date')" required :error="form.error('date')">
          <Input id="reminder-date" v-model="date" type="date" />
        </FormField>
        <FormField for="reminder-time" :label="t('reminder.time')" required :error="form.error('time')">
          <Input id="reminder-time" v-model="time" type="time" />
        </FormField>
      </div>
      <div class="grid gap-1.5" data-tour="reminder-repeat">
        <Label for="reminder-repeat">{{ t('reminder.repeat') }}</Label>
        <div class="flex items-center gap-2">
          <template v-if="unit">
            <span class="shrink-0 text-sm text-muted-foreground">{{ t('reminder.every') }}</span>
            <Input v-model="every" type="number" min="1" max="999" class="w-20" :aria-label="t('reminder.everyCount')" />
          </template>
          <Select id="reminder-repeat" v-model="unit" class="flex-1">
            <option value="">{{ t('reminder.once') }}</option>
            <option v-for="entry in REPEAT_UNITS" :key="entry" :value="entry">
              {{ t(`reminder.unit.${entry}`, Number(every) || 1) }}
            </option>
          </Select>
        </div>
        <p class="text-xs text-muted-foreground">{{ unit ? t('reminder.repeatHint') : t('reminder.onceHint') }}</p>
      </div>
      <p v-if="props.reminder?.source" class="text-xs text-muted-foreground">{{ t('reminder.autoEditHint') }}</p>
    </form>
    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="reminder-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
