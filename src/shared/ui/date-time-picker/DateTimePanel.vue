<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { DateValue } from '@internationalized/date'

import { clampToDateBounds } from '@/shared/lib/limits'
import { activeLocale } from '@/shared/lib/locale'
import { Button } from '@/shared/ui/button'
import { Calendar } from '@/shared/ui/calendar'
import { TimeInput } from '@/shared/ui/time-input'

import type { DateTimePickerProps } from './date-time-picker.types'
import { boundedDate, fromParts, nowValue, timeOf, toCalendarDate } from './date-value'
import MonthGrid from './MonthGrid.vue'

/** The calendar, clock and «today / clear / done» row of the date picker; the same in a popover and a sheet. */
const props = defineProps<DateTimePickerProps & { timeId: string }>()
const model = defineModel<string | null>({ default: '' })
const emit = defineEmits<{ close: [] }>()

const { t } = useI18n()
const value = computed(() => model.value ?? '')
const day = computed(() => toCalendarDate(value.value))
const time = computed(() => timeOf(value.value))
const weekStartsOn = computed(() => (activeLocale.value.startsWith('en') ? 0 : 1))

function pickDay(picked: DateValue | undefined) {
  if (!picked) return
  model.value = fromParts(props.mode ?? 'date', picked, time.value)
  // A date alone is done; with time the picker stays for the clock.
  if (props.mode === 'date') emit('close')
}

function pickTime(next: string | number) {
  // A time before any day means today at that time.
  const base = day.value ?? toCalendarDate(nowValue('date'))
  if (base) model.value = fromParts('datetime', base, String(next))
}

function pickMonth(next: string) {
  model.value = next
  emit('close')
}

function setNow() {
  model.value = nowValue(props.mode ?? 'date')
  emit('close')
}

function clear() {
  model.value = ''
  emit('close')
}
</script>

<template>
  <div>
    <MonthGrid
      v-if="props.mode === 'month'"
      :value="value"
      :locale="activeLocale"
      :min="clampToDateBounds(props.min, 'min')"
      :max="clampToDateBounds(props.max, 'max')"
      @pick="pickMonth"
    />
    <Calendar
      v-else
      :model-value="day"
      :locale="activeLocale"
      :week-starts-on="weekStartsOn"
      :min-value="boundedDate(props.min, 'min')"
      :max-value="boundedDate(props.max, 'max')"
      layout="month-and-year"
      @update:model-value="pickDay"
    />
    <div v-if="props.mode === 'datetime'" class="flex items-center gap-2 border-t border-border px-3 py-2">
      <label :for="`${props.timeId}-time`" class="text-sm text-muted-foreground">{{ t('ui.time') }}</label>
      <TimeInput
        :id="`${props.timeId}-time`"
        class="h-8 w-20"
        :model-value="time"
        @update:model-value="pickTime"
      />
    </div>
    <div class="flex items-center justify-between gap-2 border-t border-border px-3 py-2">
      <Button variant="ghost" size="sm" @click="setNow">
        {{ props.mode === 'datetime' ? t('ui.now') : props.mode === 'month' ? t('ui.thisMonth') : t('ui.today') }}
      </Button>
      <Button v-if="!props.required && value" variant="ghost" size="sm" class="text-muted-foreground" @click="clear">
        {{ t('ui.clear') }}
      </Button>
      <Button v-if="props.mode === 'datetime'" size="sm" @click="emit('close')">{{ t('ui.done') }}</Button>
    </div>
  </div>
</template>
