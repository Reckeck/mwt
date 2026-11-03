<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { TimeInput } from '@/shared/ui/time-input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { SyncChangedBanner } from '@/shared/ui/sync-changed-banner'
import { required, useEditBase, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { formatDayMonth, formatWeekday } from '@/shared/lib/format'

import { REPEAT_UNITS } from './reminder-edit.constants'
import type { ReminderDialogProps } from './reminder-edit.types'
import {
  type ReminderFields,
  dueAtOfFields,
  fieldsOfReminder,
  reminderBaseOf,
  reminderInputOfFields,
} from './lib/reminder-fields'
import { WHEN_PRESETS, type WhenPreset } from './lib/when-presets'
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

/** Writes fields into the form; `Refresh` and opening both go through it. */
function load(fields: ReminderFields) {
  text.value = fields.text
  date.value = fields.date
  time.value = fields.time
  preset.value = fields.preset
  unit.value = fields.unit
  every.value = fields.every
  pay.value = fields.pay
}
const currentFields = (): ReminderFields => ({
  text: text.value,
  date: date.value,
  time: time.value,
  preset: preset.value,
  unit: unit.value,
  every: every.value,
  pay: pay.value,
})

watch(open, (isOpen) => {
  if (isOpen) load(fieldsOfReminder(props.reminder))
}, { immediate: true })

/** The reminder as the sync keeps it now (`['reminders']` is re-read after `vault-synced`); the prop may be a stale copy. */
const reminders = useQuery({ queryKey: ['reminders'], queryFn: () => call<Reminder[]>('list_reminders'), enabled: open })
const edit = useEditBase({
  open,
  entity: () => props.reminder,
  live: () => (props.reminder ? reminders.data.value?.find((entry) => entry.id === props.reminder?.id) : undefined),
  model: (entity: Reminder) => fieldsOfReminder(entity),
})
function refreshFromOtherDevice() {
  edit.refresh(currentFields, load)
}

const dueAt = computed(() => dueAtOfFields({ date: date.value, time: time.value }))

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
    const input = reminderInputOfFields(currentFields())
    return props.reminder
      ? call<Reminder>('update_reminder', {
          input: { id: props.reminder.id, reminder: input, base: edit.base.value && reminderBaseOf(edit.base.value) },
        })
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
      <SyncChangedBanner v-if="edit.changedElsewhere.value" @refresh="refreshFromOtherDevice" />
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
