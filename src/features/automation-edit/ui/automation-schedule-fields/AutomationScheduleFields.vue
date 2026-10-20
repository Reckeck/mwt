<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { WEEKDAYS } from '@/shared/lib/planning'

import { SCHEDULE_KINDS, type ScheduleKind } from '../../automation-edit.constants'

/** When an automation repeats: monthly on a day, weekly on a weekday or every N days, from a date. */
const props = defineProps<{ errors: { day?: string | null; everyDays?: string | null; startsAt?: string | null } }>()
const kind = defineModel<ScheduleKind>('kind', { required: true })
const day = defineModel<string>('day', { required: true })
const weekday = defineModel<string>('weekday', { required: true })
const everyDays = defineModel<string>('everyDays', { required: true })
const startsAt = defineModel<string>('startsAt', { required: true })
const { t } = useI18n()
</script>

<template>
  <div class="grid items-start gap-3 @xl:grid-cols-3">
    <div class="grid gap-1.5">
      <Label for="automation-schedule">{{ t('automation.schedule') }}</Label>
      <Select id="automation-schedule" v-model="kind">
        <SelectItem v-for="entry in SCHEDULE_KINDS" :key="entry" :value="entry">{{ t(`schedule.kind.${entry}`) }}</SelectItem>
      </Select>
    </div>
    <FormField
      v-if="kind === 'monthly'"
      for="automation-day"
      :label="t('schedule.day')"
      required
      :error="props.errors.day ?? undefined"
      :hint="t('schedule.dayHint')"
    >
      <Input id="automation-day" v-model="day" type="text" inputmode="numeric" min="1" max="31" />
    </FormField>
    <FormField v-else-if="kind === 'weekly'" for="automation-weekday" :label="t('schedule.weekdayLabel')">
      <Select id="automation-weekday" v-model="weekday">
        <SelectItem v-for="entry in WEEKDAYS" :key="entry" :value="String(entry)">{{ t(`schedule.weekday.${entry}`) }}</SelectItem>
      </Select>
    </FormField>
    <FormField v-else for="automation-days" :label="t('schedule.days')" required :error="props.errors.everyDays ?? undefined">
      <Input id="automation-days" v-model="everyDays" type="text" inputmode="numeric" min="1" />
    </FormField>
    <FormField for="automation-start" :label="t('automation.startsAt')" required :error="props.errors.startsAt ?? undefined">
      <DateTimePicker id="automation-start" v-model="startsAt" />
    </FormField>
  </div>
</template>
