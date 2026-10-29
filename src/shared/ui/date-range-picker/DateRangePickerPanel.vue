<script setup lang="ts">
import type { DateValue } from '@internationalized/date'
import type { DateRange as CalendarRange } from 'reka-ui'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import type { DateRange } from '@/shared/lib/date-range'
import { activeLocale } from '@/shared/lib/locale'
import { Button } from '@/shared/ui/button'
import { Calendar } from '@/shared/ui/calendar'
import { fromParts, nowValue, toCalendarDate } from '@/shared/ui/date-time-picker/date-value'

import RangeCalendar from './RangeCalendar.vue'

/** The calendar and the `Today / Clear` row of the picker, shared by its popover and its sheet. */
const props = defineProps<{ range: boolean }>()
const model = defineModel<DateRange | null>({ default: null })
const emit = defineEmits<{ done: [] }>()

const { t } = useI18n()
const weekStartsOn = computed(() => (activeLocale.value.startsWith('en') ? 0 : 1))

const day = computed(() => toCalendarDate(model.value?.start))
const span = computed<CalendarRange>(() => ({
  start: toCalendarDate(model.value?.start),
  end: toCalendarDate(model.value?.end ?? model.value?.start),
}))

function pickDay(picked: DateValue | undefined) {
  if (!picked) return
  model.value = { start: fromParts('date', picked), end: null }
  emit('done')
}

function pickSpan(next: CalendarRange) {
  if (!next.start) return
  const start = fromParts('date', next.start)
  // Only the first click so far: wait for the second one.
  if (!next.end) return
  const end = fromParts('date', next.end)
  model.value = { start, end: end === start ? null : end }
  emit('done')
}

function today() {
  model.value = { start: nowValue('date'), end: null }
  emit('done')
}

function clear() {
  model.value = null
  emit('done')
}
</script>

<template>
  <RangeCalendar
    v-if="props.range"
    :model-value="span"
    :locale="activeLocale"
    :week-starts-on="weekStartsOn"
    @update:model-value="pickSpan"
  />
  <Calendar
    v-else
    :model-value="day"
    :locale="activeLocale"
    :week-starts-on="weekStartsOn"
    layout="month-and-year"
    @update:model-value="pickDay"
  />
  <div class="flex items-center justify-between gap-2 border-t border-border px-3 py-2">
    <Button variant="ghost" size="sm" @click="today">{{ t('ui.today') }}</Button>
    <Button v-if="model?.start" variant="ghost" size="sm" class="text-muted-foreground" @click="clear">
      {{ t('ui.clear') }}
    </Button>
  </div>
</template>
