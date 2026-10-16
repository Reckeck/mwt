<script setup lang="ts">
import { computed, ref, useAttrs } from 'vue'
import { useI18n } from 'vue-i18n'
import type { DateValue } from '@internationalized/date'
import { CalendarDays } from 'lucide-vue-next'

import { activeLocale } from '@/shared/lib/locale'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Calendar } from '@/shared/ui/calendar'
import { TimeInput } from '@/shared/ui/time-input'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'

import type { DateTimePickerProps } from './date-time-picker.types'
import { formatValue, fromParts, nowValue, timeOf, toCalendarDate } from './date-value'
import MonthGrid from './MonthGrid.vue'

/**
 * A date, a date with time, or a month, picked from a calendar in a popover
 * instead of the browser's own input (which looks different on every system
 * and ignores the app's language). The model is the same string a native
 * `type="date|datetime-local|month"` input holds, so it drops in for one.
 * `id`, `aria-*` and `class` go to the trigger.
 */
defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<DateTimePickerProps>(), {
  mode: 'date',
  placeholder: undefined,
  min: undefined,
  max: undefined,
})
const model = defineModel<string | null>({ default: '' })

const { t } = useI18n()
const attrs = useAttrs()
const open = ref(false)

const value = computed(() => model.value ?? '')
const shown = computed(() => formatValue(props.mode, value.value, activeLocale.value))
const day = computed(() => toCalendarDate(value.value))
const time = computed(() => timeOf(value.value))
const weekStartsOn = computed(() => (activeLocale.value.startsWith('en') ? 0 : 1))

function pickDay(picked: DateValue | undefined) {
  if (!picked) return
  model.value = fromParts(props.mode, picked, time.value)
  // A date alone is done; with time the popover stays for the clock.
  if (props.mode === 'date') open.value = false
}

function pickTime(next: string | number) {
  // A time before any day means today at that time.
  const base = day.value ?? toCalendarDate(nowValue('date'))
  if (base) model.value = fromParts('datetime', base, String(next))
}

function pickMonth(next: string) {
  model.value = next
  open.value = false
}

function setNow() {
  model.value = nowValue(props.mode)
  open.value = false
}

function clear() {
  model.value = ''
  open.value = false
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button
        variant="outline"
        :disabled="props.disabled"
        :aria-invalid="props.invalid || undefined"
        v-bind="attrs"
        :class="
          cn(
            'w-full justify-start gap-2 px-3 font-normal',
            !shown && 'text-muted-foreground',
            props.invalid && 'border-destructive',
            attrs.class as string,
          )
        "
      >
        <CalendarDays class="size-4 shrink-0 text-muted-foreground" />
        <span class="truncate">{{ shown || props.placeholder || t(`ui.pick.${props.mode}`) }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverContent class="w-auto p-0" align="start">
      <MonthGrid
        v-if="props.mode === 'month'"
        :value="value"
        :locale="activeLocale"
        :min="props.min"
        :max="props.max"
        @pick="pickMonth"
      />
      <Calendar
        v-else
        :model-value="day"
        :locale="activeLocale"
        :week-starts-on="weekStartsOn"
        :min-value="toCalendarDate(props.min)"
        :max-value="toCalendarDate(props.max)"
        layout="month-and-year"
        @update:model-value="pickDay"
      />
      <div v-if="props.mode === 'datetime'" class="flex items-center gap-2 border-t border-border px-3 py-2">
        <label :for="`${attrs.id ?? 'dt'}-time`" class="text-sm text-muted-foreground">{{ t('ui.time') }}</label>
        <TimeInput
          :id="`${attrs.id ?? 'dt'}-time`"
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
        <Button v-if="props.mode === 'datetime'" size="sm" @click="open = false">{{ t('ui.done') }}</Button>
      </div>
    </PopoverContent>
  </Popover>
</template>
