<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

import { HEAT_CLASSES } from '../spending-calendar.constants'

/**
 * One day of the month: its heat (the day's spending against the month's heaviest day), the money
 * that came in and went out, and the leading category. On a phone only the number and the heat.
 */
const props = defineProps<{
  day: CalendarDay
  currency: string
  today: string
  selected: boolean
  heat: number
  compact: boolean
}>()
const emit = defineEmits<{ pick: [] }>()
const { t } = useI18n()
const categories = useCategories()

const isToday = computed(() => props.day.date === props.today)
const income = computed(() => Number(props.day.income) > 0)
const spent = computed(() => Number(props.day.spent) > 0)
const label = computed(() => {
  if (props.day.lead_category) return categoryKeyLabel(props.day.lead_category, categories.data.value, t)
  return props.day.lead_kind ? t(`tx.kindNoun.${props.day.lead_kind}`) : null
})
</script>

<template>
  <Button
    variant="ghost"
    size="block"
    :class="
      cn(
        'h-auto min-w-0 flex-col items-stretch justify-start gap-0.5 rounded-none px-2 py-1.5 text-left font-normal',
        HEAT_CLASSES[props.heat],
        props.compact ? 'min-h-11 items-center' : 'min-h-[92px]',
        props.day.forecast && 'opacity-60',
        (isToday && !props.compact) || props.selected ? 'shadow-[inset_0_0_0_2px_var(--color-primary)]' : '',
      )
    "
    :aria-pressed="props.selected"
    :aria-current="isToday ? 'date' : undefined"
    data-calendar-day
    :data-heat="props.heat"
    @click="emit('pick')"
  >
    <span class="flex w-full items-center gap-1 text-sm">
      {{ Number(props.day.date.slice(8)) }}
      <span v-if="isToday" class="sr-only">{{ t('calendar.today') }}</span>
      <span v-if="props.day.status === 'over' && !props.compact" class="ml-auto size-1.5 rounded-full bg-destructive" :title="t('calendar.overDay')" />
    </span>
    <template v-if="!props.compact">
      <span v-if="props.day.tx_count && label" class="max-w-full truncate text-2xs opacity-80">{{ label }}</span>
      <span v-if="income" class="truncate text-xs tabular-nums">
        +<Money :amount="props.day.income" :decimals="0" :currency="props.currency" />
      </span>
      <span v-if="spent" class="truncate text-xs tabular-nums">
        −<Money :amount="props.day.spent" :decimals="0" :currency="props.currency" />
      </span>
    </template>
  </Button>
</template>
