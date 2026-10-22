<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Money } from '@/entities/currency'

import { buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { useCategories } from '@/shared/api/queries'
import { SectionState } from '@/shared/ui/section-state'
import { ProgressBar } from '@/shared/ui/progress-bar'

import { byRisk } from './model/budget-pace'

const { t } = useI18n()

const statuses = useQuery({ queryKey: ['budget-status'], queryFn: () => call<BudgetStatus[]>('get_budget_status') })
const categories = useCategories()

function label(target: BudgetTarget): string {
  if (target === 'total') return t('budget.targetKind.total')
  if ('category' in target) return categoryKeyLabel(target.category, categories.data.value, t)
  return t('budget.targetKind.store')
}
const pct = (status: BudgetStatus) => Math.min(100, Math.round(Number(status.used) * 100))
const paces = computed(() => byRisk(statuses.data.value ?? []))
</script>

<template>
  <Card size="sm" v-if="statuses.data.value?.length">
    <template #header>
      <RouterLink data-widget-title :to="{ name: 'planning-budgets' }" class="text-sm text-muted-foreground hover:text-foreground">
        {{ t('budget.dashboardTitle') }}
      </RouterLink>
    </template>
    <ul class="grid gap-3">
      <li v-for="{ status, elapsed, forecast, risk } in paces" :key="status.budget.id" class="grid gap-1">
        <div class="flex items-center justify-between gap-2 text-sm">
          <span class="min-w-0 truncate">{{ label(status.budget.target) }} · {{ t(`budget.periodKind.${status.budget.period}`) }}</span>
          <span class="shrink-0 tabular-nums" :class="Number(status.remaining) < 0 ? 'text-destructive' : 'text-muted-foreground'">
            <Money :amount="status.spent" :decimals="0" :currency="status.budget.currency" /> / <Money :amount="status.budget.limit" :decimals="0" :currency="status.budget.currency" />
          </span>
        </div>
        <ProgressBar
          :value="pct(status)"
          size="sm"
          :tone="Number(status.remaining) < 0 ? 'destructive' : risk > 1 ? 'warning' : 'primary'"
        />
        <p class="text-xs tabular-nums" :class="risk > 1 ? 'text-warning' : 'text-muted-foreground'">
          <template v-if="Number(status.remaining) >= 0 && risk > 1">
            {{ t('budget.paceOver') }}
            <Money :amount="forecast - Number(status.budget.limit)" :decimals="0" :currency="status.budget.currency" />
          </template>
          <template v-else-if="Number(status.remaining) >= 0">
            {{ t('budget.paceOk', { pct: Math.round(elapsed * 100) }) }}
          </template>
        </p>
      </li>
    </ul>
  </Card>
  <Card size="sm" v-else-if="statuses.isPending.value"><Skeleton class="h-24 w-full" /></Card>
  <!-- A failed load is not «nothing here»: it said «create one» to someone who has several. -->
  <Card size="sm" v-else-if="statuses.error.value">
    <SectionState :error="statuses.error.value" compact @retry="statuses.refetch()" />
  </Card>
  <Card size="sm" v-else>
    <div class="flex flex-col items-start gap-2">
      <p class="text-sm text-muted-foreground">{{ t('dashboard.widgetEmpty') }}</p>
      <RouterLink :to="{ name: 'planning-budgets' }" :class="buttonVariants({ variant: 'outline', size: 'sm' })">
        {{ t('budget.new') }}
      </RouterLink>
    </div>
  </Card>
</template>
