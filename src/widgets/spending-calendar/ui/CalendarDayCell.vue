<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

/**
 * One day of the month: what happened that day (the leading operation's kind
 * or category and the day's sum) or, ahead of today, what each day may spend.
 * Today says how much is left to spend today. On a phone only dots.
 */
const props = defineProps<{
  day: CalendarDay
  currency: string
  today: string
  selected: boolean
  /** Today's «можно»: the day's allowance plus what earlier days left unspent. */
  leftToday: string | null
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
        'h-auto min-w-0 flex-col items-stretch justify-start gap-0.5 rounded-none bg-card px-2 py-1.5 text-left font-normal hover:bg-muted/60',
        props.compact ? 'min-h-11 items-center' : 'min-h-[92px]',
        (isToday && !props.compact) || props.selected ? 'shadow-[inset_0_0_0_2px_var(--color-primary)]' : '',
      )
    "
    :aria-pressed="props.selected"
    :aria-current="isToday ? 'date' : undefined"
    data-calendar-day
    @click="emit('pick')"
  >
    <span class="flex items-center gap-1 text-sm">
      {{ Number(props.day.date.slice(8)) }}
      <span v-if="isToday" class="sr-only">{{ t('calendar.today') }}</span>
      <span
        v-if="props.day.status === 'over' && !props.compact"
        class="ml-auto size-1.5 rounded-full bg-warning"
        :title="t('calendar.overDay')"
      />
    </span>
    <template v-if="props.compact">
      <span v-if="income || spent || props.day.tx_count" class="size-1.5 rounded-full" :class="income ? 'bg-success' : 'bg-primary'" />
    </template>
    <template v-else>
      <span
        v-if="props.day.tx_count && label"
        class="max-w-full truncate rounded bg-secondary px-1 text-2xs"
        :class="income && !spent && 'text-success'"
      >
        {{ label }}
      </span>
      <span v-if="income" class="truncate text-xs tabular-nums text-success">
        +<Money :amount="props.day.income" :decimals="0" :currency="props.currency" />
      </span>
      <!-- A day of own transfers alone spent nothing: the kind's chip says what happened, no «0,00» as an expense. -->
      <span v-if="spent" class="truncate text-xs tabular-nums text-muted-foreground">
        −<Money :amount="props.day.spent" :currency="props.currency" />
      </span>
      <span v-if="isToday && props.leftToday !== null" class="truncate text-xs font-medium text-primary">
        {{ t('calendar.canToday') }} <Money :amount="props.leftToday" :decimals="0" :currency="props.currency" />
      </span>
      <span v-else-if="props.day.forecast && props.day.per_day_left" class="truncate text-xs tabular-nums text-muted-foreground/55">
        <Money :amount="props.day.per_day_left" :decimals="0" :currency="props.currency" />
      </span>
    </template>
  </Button>
</template>
