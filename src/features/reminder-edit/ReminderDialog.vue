<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { TimeInput } from '@/shared/ui/time-input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS, localDate } from '@/shared/lib/planning'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { formatDayMonth, formatWeekday } from '@/shared/lib/format'

import { REPEAT_UNITS } from './reminder-edit.constants'
import type { ReminderDialogProps } from './reminder-edit.types'
import { WHEN_PRESETS, type WhenPreset, presetOf, repeatOf } from './lib/when-presets'
import { ReminderPayField } from './ui/reminder-pay-field'

const props = withDefaults(defineProps<ReminderDialogProps>(), { reminder: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const text = ref('')
/** `YYYY-MM-DD` and `HH:MM`, local: two fields read better than one date-time picker. */
const date = ref('')
const time = ref('09:00')
const preset = ref<WhenPreset>('once')
/** `Other interval`: every `every` `unit`s. */
const unit = ref<RepeatUnit>('month')
const every = ref('1')
const pay = ref<ReminderPay | null>(null)

const pad = (value: number) => String(value).padStart(2, '0')

watch(open, (isOpen) => {
  if (!isOpen) return
  const reminder = props.reminder
  text.value = reminder?.text ?? ''
  const due = reminder ? new Date(reminder.due_at) : new Date(Date.now() + 86_400_000)
  date.value = localDate(due)
  time.value = reminder ? `${pad(due.getHours())}:${pad(due.getMinutes())}` : '09:00'
  preset.value = presetOf(reminder?.repeat)
  unit.value = reminder?.repeat?.unit ?? 'month'
  every.value = String(reminder?.repeat?.every ?? 1)
  pay.value = reminder?.pay ?? null
}, { immediate: true })

const dueAt = computed(() => {
  if (!date.value || !time.value) return null
  const parsed = new Date(`${date.value}T${time.value}`)
  return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString()
})
const repeat = computed(() => repeatOf(preset.value, { unit: unit.value, every: Number(every.value) }))

/** `Every 10th`, `Every week · Fri`: the day comes from the date. */
function presetLabel(entry: WhenPreset) {
  const day = date.value ? new Date(`${date.value}T12:00`) : null
  if (!day || Number.isNaN(day.getTime())) return t(`reminder.when.${entry}`, { day: '', date: '' })
  return t(`reminder.when.${entry}`, { day: day.getDate(), weekday: formatWeekday(day), date: formatDayMonth(date.value) })
}

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
    const input: ReminderInput = { text: text.value.trim(), due_at: dueAt.value ?? '', repeat: repeat.value, pay: pay.value ?? undefined }
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
  <Dialog overlay="reminder" v-model:open="open" size="form" :title="props.reminder ? t('reminder.edit') : t('reminder.new')">
    <form id="reminder-form" class="grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <FormField for="reminder-text" :label="t('reminder.text')" required :error="form.error('text')">
        <Input id="reminder-text" v-model="text" />
      </FormField>
      <div class="grid gap-4 sm:grid-cols-2" data-tour="reminder-repeat">
        <FormField for="reminder-when" :label="t('reminder.dueAt')" required>
          <Select id="reminder-when" v-model="preset">
            <SelectItem v-for="entry in WHEN_PRESETS" :key="entry" :value="entry">{{ presetLabel(entry) }}</SelectItem>
          </Select>
        </FormField>
        <FormField for="reminder-time" :label="t('reminder.time')" required :error="form.error('time')">
          <TimeInput id="reminder-time" v-model="time" :invalid="Boolean(form.error('time'))" />
        </FormField>
      </div>
      <div class="grid gap-4 sm:grid-cols-2">
        <FormField
          for="reminder-date"
          :label="preset === 'once' ? t('reminder.date') : t('reminder.firstDate')"
          required
          :error="form.error('date')"
        >
          <DateTimePicker id="reminder-date" v-model="date" />
        </FormField>
        <div v-if="preset === 'custom'" class="grid gap-1.5">
          <Label for="reminder-unit">{{ t('reminder.repeat') }}</Label>
          <div class="flex items-center gap-2">
            <span class="shrink-0 text-sm text-muted-foreground">{{ t('reminder.every') }}</span>
            <Input v-model="every" type="text" inputmode="numeric" class="w-16" :aria-label="t('reminder.everyCount')" />
            <Select id="reminder-unit" v-model="unit" class="flex-1">
              <SelectItem v-for="entry in REPEAT_UNITS" :key="entry" :value="entry">{{ t(`reminder.unit.${entry}`, Number(every) || 1) }}</SelectItem>
            </Select>
          </div>
        </div>
      </div>
      <ReminderPayField v-model="pay" />
      <p v-if="props.reminder?.source" class="text-xs text-muted-foreground">{{ t('reminder.autoEditHint') }}</p>
    </form>
    <template #footer>
      <Button variant="ghost" class="max-md:h-11" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button class="max-md:h-11 max-md:flex-1" form="reminder-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
