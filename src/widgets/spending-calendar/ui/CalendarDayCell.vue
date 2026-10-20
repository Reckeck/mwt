<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Bell, BellRing, CalendarClock } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { numberFormat } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'

import { STATUS_CLASSES } from '../spending-calendar.constants'

/**
 * One day of the month: its date and what happened — spent, received, the
 * month's rest after it (a spreadsheet's running balance), and marks for
 * notifications, fired price alerts and payments still to come.
 */
const props = defineProps<{
  day: CalendarDay
  currency: string
  today: string
  selected: boolean
  /** Payments and reminders due that day, from the month's «upcoming». */
  upcoming: number
  /** A seventh of a phone is ~45px: sums go compact, the rest is in the day panel. */
  compact: boolean
}>()
const emit = defineEmits<{ pick: [] }>()
const { t } = useI18n()

const alerts = computed(() => props.day.notices.filter((notice) => notice.category === 'price_alert').length)
const others = computed(() => props.day.notices.length - alerts.value)
/** A narrow cell: the bare number (`2,8 тыс.`), the currency is in the month's header. */
const compactLeft = computed(() => {
  const value = Number(props.day.left ?? 0)
  return numberFormat({ notation: 'compact', maximumFractionDigits: 1 }).format(Math.abs(value) < 0.005 ? 0 : value)
})
</script>

<template>
  <Button
    variant="outline"
    size="block"
    class="grid h-auto min-h-16 min-w-0 content-start gap-0.5 overflow-hidden rounded-md border p-1.5 text-left text-xs transition-colors"
    :class="[
      STATUS_CLASSES[props.day.status],
      props.selected ? 'ring-2 ring-primary' : props.day.date === props.today ? 'ring-1 ring-primary' : '',
    ]"
    :aria-pressed="props.selected"
    :aria-current="props.day.date === props.today ? 'date' : undefined"
    @click="emit('pick')"
  >
    <span class="flex w-full items-center gap-1">
      <span class="font-medium">{{ Number(props.day.date.slice(8)) }}</span>
      <span v-if="props.day.date === props.today" class="sr-only">{{ t('calendar.today') }}</span>
      <span class="ml-auto flex items-center gap-0.5 text-muted-foreground">
        <Bell v-if="others" class="size-3" :aria-label="t('calendar.mark.notices', { count: others })" />
        <BellRing v-if="alerts" class="size-3 text-warning" :aria-label="t('calendar.mark.alerts', { count: alerts })" />
        <CalendarClock v-if="props.upcoming" class="size-3" :aria-label="t('calendar.mark.upcoming', { count: props.upcoming })" />
      </span>
    </span>
    <span v-if="Number(props.day.daily)" class="truncate tabular-nums">
      −<Money :amount="props.day.daily" :decimals="0" :compact="props.compact" :currency="props.currency" />
    </span>
    <span v-if="Number(props.day.income)" class="truncate tabular-nums text-success">
      +<Money :amount="props.day.income" :decimals="0" :compact="props.compact" :currency="props.currency" />
    </span>
    <!-- The month's rest after this day; ahead of today the forecast if the plan is kept («≈»). -->
    <span
      v-if="props.day.left !== null"
      class="truncate text-2xs tabular-nums text-muted-foreground"
      :class="[Number(props.day.left) < 0 && 'text-destructive', props.day.forecast && 'opacity-70']"
      :title="props.day.forecast ? t('calendar.forecastHint') : t('calendar.leftMonth')"
    >
      <template v-if="props.compact">{{ props.day.forecast ? '≈' : '' }}{{ compactLeft }}</template>
      <template v-else>
        {{ props.day.forecast ? '≈' : t('calendar.leftShort') }}
        <Money :amount="props.day.left" :decimals="0" :currency="props.currency" :convert="false" />
      </template>
    </span>
  </Button>
</template>
