<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatDay, formatMonthKey } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { cn } from '@/shared/lib/utils'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'

import { PERIOD_CHIPS, monthDays, monthOfDays, presetDays, presetOfDays, recentMonths } from '../../lib/period-field'
import type { FilterFieldProps } from '../../transaction-filter.types'

/**
 * `Period`: a month from the list, or a chip — this month, the last 3…12, the
 * year, or `from … to` with two days. The filter holds days; whole months are
 * read back from them, so a period from a report shows as its chip.
 */
const props = defineProps<FilterFieldProps>()
const filter = defineModel<TxFilter>({ required: true })

const { t } = useI18n()
const months = recentMonths()
const RANGE = 'range'

const preset = computed(() => presetOfDays(filter.value.from, filter.value.to))
const month = computed(() => monthOfDays(filter.value.from, filter.value.to))
/** Days that are no month and no chip: `from … to` is open. */
const custom = ref(false)
watch(
  () => [filter.value.from, filter.value.to] as const,
  ([from, to]) => {
    if ((from || to) && !preset.value && !month.value) custom.value = true
  },
  { immediate: true },
)

const monthName = (key: string) => {
  const name = formatMonthKey(key, { month: 'long' })
  return `${name.charAt(0).toUpperCase()}${name.slice(1)} ${key.slice(0, 4)}`
}
const rangeLabel = computed(() => `${formatDay(filter.value.from)} – ${formatDay(filter.value.to)}`)

const selected = computed({
  get: () => month.value || (filter.value.from || filter.value.to ? RANGE : ''),
  set: (key: string) => {
    if (key === RANGE) return
    custom.value = false
    filter.value = { ...filter.value, ...(key ? monthDays(key) : { from: null, to: null }) }
  },
})

function pick(chip: (typeof PERIOD_CHIPS)[number] | typeof RANGE) {
  if (chip === RANGE) {
    custom.value = true
    return
  }
  custom.value = false
  filter.value = { ...filter.value, ...presetDays(chip) }
}

const day = (name: 'from' | 'to') =>
  computed({
    get: () => filter.value[name] ?? '',
    set: (value: string) => {
      filter.value = { ...filter.value, [name]: value || null }
    },
  })
const from = day('from')
const to = day('to')

const chipClass = (on: boolean) =>
  cn(
    'touch-hit inline-flex h-6 items-center rounded-[7px] border px-2 text-xs font-normal transition-colors',
    on ? 'border-foreground bg-foreground text-background' : 'border-border text-muted-foreground hover:text-foreground',
  )
</script>

<template>
  <div class="grid min-w-0 content-start gap-1.5">
    <Label :for="`${props.idPrefix}-period`">{{ t('filter.period') }}</Label>
    <Select :id="`${props.idPrefix}-period`" v-model="selected">
      <SelectItem value="">{{ t('period.preset.all') }}</SelectItem>
      <SelectItem v-if="selected === RANGE" :value="RANGE">{{ rangeLabel }}</SelectItem>
      <SelectItem v-for="key in months" :key="key" :value="key">{{ monthName(key) }}</SelectItem>
    </Select>
    <fieldset class="m-0 flex min-w-0 flex-wrap gap-1.5 border-0 p-0" :aria-label="t('filter.period')">
      <Button
        v-for="chip in PERIOD_CHIPS"
        :key="chip"
        variant="ghost"
        size="sm"
        :class="chipClass(preset === chip && !custom)"
        :aria-pressed="preset === chip && !custom"
        @click="pick(chip)"
      >
        {{ t(`filter.chip.${chip}`) }}
      </Button>
      <Button variant="ghost" size="sm" :class="chipClass(custom)" :aria-pressed="custom" @click="pick(RANGE)">
        {{ t('filter.chip.range') }}
      </Button>
    </fieldset>
    <div v-if="custom" class="grid grid-cols-2 gap-2">
      <DateTimePicker :id="`${props.idPrefix}-from`" v-model="from" :max="to || undefined" :aria-label="t('filter.from')" />
      <DateTimePicker :id="`${props.idPrefix}-to`" v-model="to" :min="from || undefined" :aria-label="t('filter.to')" />
    </div>
  </div>
</template>
