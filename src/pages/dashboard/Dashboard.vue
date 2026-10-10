<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { DailyBudget } from '@/widgets/daily-budget'
import { DashboardAllocation } from '@/widgets/dashboard-allocation'
import { DashboardAssets } from '@/widgets/dashboard-assets'
import { DashboardBudgets } from '@/widgets/dashboard-budgets'
import { DashboardFx } from '@/widgets/dashboard-fx'
import { DashboardGoals } from '@/widgets/dashboard-goals'
import { DashboardTotal } from '@/widgets/dashboard-total'
import { MiniCalendar } from '@/widgets/mini-calendar'
import { MonthSummary } from '@/widgets/month-summary'
import { NetWorthCard } from '@/widgets/net-worth'
import { OnboardingChecklist } from '@/widgets/onboarding-checklist'
import { Spending7d } from '@/widgets/spending-7d'
import { UpcomingList } from '@/widgets/upcoming-list'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settingsStore = useSettingsStore()

// Dashboard widget definitions matching backend
const widgetDefs = [
  { id: 'total', defaultOrder: 0 },
  { id: 'assets', defaultOrder: 1 },
  { id: 'fx', defaultOrder: 2 },
  { id: 'byChain', defaultOrder: 3 },
  { id: 'byStore', defaultOrder: 4 },
  { id: 'dailyBudget', defaultOrder: 5 },
  { id: 'monthSummary', defaultOrder: 6 },
  { id: 'upcoming', defaultOrder: 7 },
  { id: 'goals', defaultOrder: 8 },
  { id: 'budgets', defaultOrder: 9 },
  { id: 'spending7d', defaultOrder: 10 },
  { id: 'miniCalendar', defaultOrder: 11 },
  { id: 'netWorth', defaultOrder: 12 },
] as const

const visibleWidgets = computed(() => {
  const widgets = settingsStore.vault?.dashboard_widgets ?? []
  return widgetDefs
    .map((def) => {
      const existing = widgets.find((widget) => widget.id === def.id)
      return {
        id: def.id,
        visible: existing?.visible ?? true,
        order: existing?.order ?? def.defaultOrder,
      }
    })
    .filter((widget) => widget.visible)
    .sort((left, right) => left.order - right.order)
})

const isVisible = (id: (typeof widgetDefs)[number]['id']) => visibleWidgets.value.some((widget) => widget.id === id)
</script>

<template>
  <div class="grid gap-6 @container">
    <OnboardingChecklist />
    <!-- Budget first (plan §9.9): today, this month, what is coming -->
    <div class="grid items-start gap-4 @sm:grid-cols-2 @lg:grid-cols-3">
      <DailyBudget v-if="isVisible('dailyBudget')" />
      <MonthSummary v-if="isVisible('monthSummary')" />
      <UpcomingList v-if="isVisible('upcoming')" :days="14" :limit="6" />
    </div>
    <div class="grid items-start gap-4 @sm:grid-cols-2 @lg:grid-cols-3">
      <Spending7d v-if="isVisible('spending7d')" />
      <MiniCalendar v-if="isVisible('miniCalendar')" />
      <NetWorthCard v-if="isVisible('netWorth')" />
    </div>
    <div class="grid items-start gap-4 @sm:grid-cols-2">
      <DashboardGoals v-if="isVisible('goals')" />
      <DashboardBudgets v-if="isVisible('budgets')" />
    </div>
    <!-- Top row (total, assets, fx): 3 columns on lg, 2 on sm -->
    <div class="grid gap-4 @sm:grid-cols-2 @lg:grid-cols-3">
      <DashboardTotal v-if="isVisible('total')" />
      <DashboardAssets v-if="isVisible('assets')" />
      <DashboardFx v-if="isVisible('fx')" />
    </div>

    <!-- Bottom row (by chain, by store): 2 columns on sm -->
    <div class="grid gap-4 @sm:grid-cols-2">
      <DashboardAllocation v-if="isVisible('byChain')" by="chain" />
      <DashboardAllocation v-if="isVisible('byStore')" by="store" />
    </div>

    <!-- Empty state when no widgets are visible -->
    <div v-if="!visibleWidgets.length" class="py-12 text-center text-sm text-muted-foreground">
      {{ t('store.empty') }}
    </div>
  </div>
</template>
