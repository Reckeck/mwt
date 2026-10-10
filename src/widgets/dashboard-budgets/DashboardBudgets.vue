<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney } from '@/shared/lib/format'

const { t } = useI18n()

const statuses = useQuery({ queryKey: ['budget-status'], queryFn: () => call<BudgetStatus[]>('get_budget_status') })
const categories = useQuery({ queryKey: ['categories'], queryFn: () => call<Category[]>('list_categories') })

function label(target: BudgetTarget): string {
  if (target === 'total') return t('budget.targetKind.total')
  if ('category' in target) return categoryKeyLabel(target.category, categories.data.value, t)
  return t('budget.targetKind.store')
}
const pct = (status: BudgetStatus) => Math.min(100, Math.round(Number(status.used) * 100))
</script>

<template>
  <Card v-if="statuses.data.value?.length">
    <template #header>
      <RouterLink :to="{ name: 'planning-budgets' }" class="text-sm text-muted-foreground hover:text-foreground">
        {{ t('budget.dashboardTitle') }}
      </RouterLink>
    </template>
    <ul class="grid gap-3">
      <li v-for="status in statuses.data.value" :key="status.budget.id" class="grid gap-1">
        <div class="flex items-center justify-between gap-2 text-sm">
          <span class="min-w-0 truncate">{{ label(status.budget.target) }} · {{ t(`budget.periodKind.${status.budget.period}`) }}</span>
          <span class="shrink-0 tabular-nums" :class="Number(status.remaining) < 0 ? 'text-destructive' : 'text-muted-foreground'">
            {{ formatMoney(status.spent, { decimals: 0 }) }} / {{ formatMoney(status.budget.limit, { decimals: 0, currency: status.budget.currency }) }}
          </span>
        </div>
        <div class="h-1.5 overflow-hidden rounded-full bg-muted">
          <div
            class="h-full rounded-full"
            :class="Number(status.remaining) < 0 ? 'bg-destructive' : 'bg-primary'"
            :style="{ width: `${pct(status)}%` }"
          />
        </div>
      </li>
    </ul>
  </Card>
</template>
