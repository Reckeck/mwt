<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'

/** Under the grid: the month's totals and what the pale sums and the orange dot mean. */
const props = defineProps<{
  currency: string
  income: number
  spent: number
  perDay: string | null
  overDays: number
}>()
const { t } = useI18n()
</script>

<template>
  <div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
    <span>
      <b class="font-semibold text-success">+<Money :amount="String(props.income)" :decimals="0" :currency="props.currency" /></b>
      {{ t('calendar.legend.income') }}
    </span>
    <span>−<Money :amount="String(props.spent)" :decimals="0" :currency="props.currency" /> {{ t('calendar.legend.spent') }}</span>
    <span v-if="props.perDay !== null">
      <span class="opacity-55"><Money :amount="props.perDay" :decimals="0" :currency="props.currency" :convert="false" /></span>
      — {{ t('calendar.legend.perDay') }}
    </span>
    <span class="inline-flex items-center gap-1">
      <span class="size-1.5 rounded-full bg-warning" aria-hidden="true" />
      {{ props.overDays ? t('calendar.legend.over', { count: props.overDays }) : t('calendar.legend.overNone') }}
    </span>
    <RouterLink :to="{ name: 'money-month' }" class="ml-auto text-primary hover:underline">{{ t('calendar.legend.month') }}</RouterLink>
  </div>
</template>
