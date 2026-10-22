<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { ProgressBar } from '@/shared/ui/progress-bar'

/**
 * The month's pace (plan H3): spent by today against what the plan lets spend
 * by today, and where today's pace ends the month.
 */
const props = defineProps<{ pace: MonthPace }>()
const { t } = useI18n()

const spent = computed(() => Number(props.pace.spent.value))
const plan = computed(() => (props.pace.spend_plan_to_date === null ? null : Number(props.pace.spend_plan_to_date)))
const ratio = computed(() => (plan.value ? (spent.value / plan.value) * 100 : 0))
/** What stays at the month's end at today's pace, after the savings plan. */
const leftAtEnd = computed(
  () => Number(props.pace.income.value) - Number(props.pace.forecast_spent) - Number(props.pace.savings_plan),
)
</script>

<template>
  <div v-if="plan !== null" class="grid gap-1.5 text-xs" data-tour="dash-month-pace">
    <div class="flex items-baseline justify-between gap-2 tabular-nums">
      <span class="text-muted-foreground">{{ t('dailyBudget.pace') }}</span>
      <span>
        <Money :amount="pace.spent.value" :currency="pace.currency" :decimals="0" />
        /
        <Money :amount="pace.spend_plan_to_date ?? '0'" :currency="pace.currency" :decimals="0" />
      </span>
    </div>
    <ProgressBar
      :value="ratio"
      size="sm"
      :tone="ratio > 100 ? 'destructive' : ratio > 90 ? 'warning' : 'primary'"
      :label="t('dailyBudget.pace')"
    />
    <p class="text-muted-foreground">
      {{ t(leftAtEnd < 0 ? 'dailyBudget.forecastShort' : 'dailyBudget.forecastLeft') }}
      <Money
        :amount="Math.abs(leftAtEnd)"
        :currency="pace.currency"
        :decimals="0"
        :class="leftAtEnd < 0 ? 'text-destructive' : 'text-foreground'"
      />
    </p>
  </div>
</template>
