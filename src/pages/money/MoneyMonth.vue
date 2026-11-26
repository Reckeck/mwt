<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { BudgetList } from '@/widgets/budget-list'
import { DailyBudget } from '@/widgets/daily-budget'
import { MonthSummary } from '@/widgets/month-summary'
import { currentMonth } from '@/shared/lib/period'

import MonthBalanceFigures from './ui/month/MonthBalanceFigures.vue'

/**
 * The month (Budget): what is left to spend and the pace, how it went, and the categories against their
 * limits as Budget rows. It is always the running month: the figures above it are the backend's «now».
 */
const { t } = useI18n()
const month = currentMonth()
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <div class="grid items-start gap-4 @3xl:grid-cols-2">
      <div class="grid gap-4" data-tour="month-left">
        <DailyBudget />
        <MonthSummary />
      </div>
      <div class="grid gap-4">
        <MonthBalanceFigures :month="month" />
        <section class="grid gap-2 rounded-lg border border-border bg-card p-4">
          <h2 class="heading-section">{{ t('money.month.limits') }}</h2>
          <BudgetList />
        </section>
      </div>
    </div>
  </div>
</template>
