<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { CalendarDays } from 'lucide-vue-next'

import { localDay, toLocalInput } from '@/shared/lib/datetime'
import { Button } from '@/shared/ui/button'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

/**
 * The day of a quick operation as chips (board 533927): «Сегодня» with the calendar, «Вчера» and
 * «Другая дата», which opens the date and time picker; the lit one is dark. Today and yesterday keep
 * the time of day the form holds. The model is the local `YYYY-MM-DDTHH:mm`, blank meaning now.
 */
const props = defineProps<{ id?: string; invalid?: boolean }>()
const model = defineModel<string>({ default: '' })
const { t } = useI18n()

const DAY_MS = 86_400_000
const yesterday = () => localDay(new Date(Date.now() - DAY_MS))
const day = computed(() => (model.value ? model.value.slice(0, 10) : localDay()))
const which = computed(() => {
  if (day.value === localDay()) return 'today'
  return day.value === yesterday() ? 'yesterday' : 'other'
})
/** Only a day other than today and yesterday shows in the picker chip. */
const other = computed({
  get: () => (which.value === 'other' ? model.value : ''),
  set: (value: string | null) => {
    model.value = value ?? ''
  },
})

function pickDay(target: string) {
  const time = (model.value || toLocalInput(new Date().toISOString())).slice(11, 16)
  model.value = `${target}T${time}`
}

const chip = 'h-9 gap-1.5 rounded-full px-3 text-sm font-normal [&_svg]:size-4'
</script>

<template>
  <Button
    type="button"
    :class="chip"
    :variant="which === 'today' ? 'default' : 'outline'"
    :aria-pressed="which === 'today'"
    data-date-chip="today"
    @click="pickDay(localDay())"
  >
    <CalendarDays aria-hidden="true" />{{ t('quickAdd.today') }}
  </Button>
  <Button
    type="button"
    :class="chip"
    :variant="which === 'yesterday' ? 'default' : 'outline'"
    :aria-pressed="which === 'yesterday'"
    data-date-chip="yesterday"
    @click="pickDay(yesterday())"
  >
    {{ t('quickAdd.yesterday') }}
  </Button>
  <DateTimePicker
    :id="props.id"
    v-model="other"
    chip
    mode="datetime"
    :placeholder="t('quickAdd.otherDate')"
    :invalid="props.invalid"
    :class="which === 'other' && 'border-primary bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground'"
  />
</template>
