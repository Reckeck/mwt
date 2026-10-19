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
import { formatDate, } from '@/shared/lib/format'

const { t } = useI18n()

const budget = useQuery({ queryKey: ['daily-budget'], queryFn: () => call<DailyBudget>('get_daily_budget') })
const data = computed(() => budget.data.value ?? null)
/** Nothing to spread yet: no income, fact or plan, this month. */
const unset = computed(
  () => data.value !== null && data.value.mode === 'month' && Number(data.value.income) === 0,
)
</script>

<template>
  <Card size="sm" v-if="data">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p data-widget-title class="text-sm text-muted-foreground">{{ t('dailyBudget.title') }}</p>
        <span class="text-xs text-muted-foreground">{{ t(`settings.dailyBudget.${data.mode}`) }}</span>
      </div>
    </template>

    <div v-if="unset" class="grid gap-2">
      <p class="text-sm text-muted-foreground">{{ t('dailyBudget.noIncome') }}</p>
      <RouterLink :to="{ name: 'planning-automations', query: { new: 'salary' } }" :class="buttonVariants({ variant: 'outline', size: 'sm' })" class="w-fit">
        {{ t('dailyBudget.addSalary') }}
      </RouterLink>
    </div>
    <div v-else class="grid gap-3">
      <div>
        <p class="text-xs text-muted-foreground">{{ t('dailyBudget.leftToday') }}</p>
        <p
          class="text-2xl font-semibold tabular-nums @xs:text-3xl"
          :class="Number(data.left_today) < 0 && 'text-destructive'"
        >
          <Money :amount="data.left_today" :decimals="0" :currency="data?.currency" />
        </p>
      </div>
      <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs tabular-nums">
        <dt class="text-muted-foreground">{{ t('dailyBudget.perDay') }}</dt>
        <dd class="text-right"><Money :amount="data.per_day" :decimals="0" :currency="data?.currency" /></dd>
        <template v-if="data.mode === 'month'">
          <dt class="text-muted-foreground">{{ t('dailyBudget.carry') }}</dt>
          <dd class="text-right" :class="Number(data.carry) < 0 ? 'text-destructive' : ''"><Money :amount="data.carry" :decimals="0" :currency="data?.currency" /></dd>
        </template>
        <template v-else>
          <dt class="text-muted-foreground">{{ t('dailyBudget.free') }}</dt>
          <dd class="text-right"><Money :amount="data.free" :decimals="0" :currency="data?.currency" /></dd>
          <dt class="text-muted-foreground">{{ t('dailyBudget.until') }}</dt>
          <dd class="text-right">{{ data.next_income ? formatDate(data.next_income) : t('dailyBudget.monthEnd') }}</dd>
        </template>
        <dt class="text-muted-foreground">{{ t('dailyBudget.spentToday') }}</dt>
        <dd class="text-right"><Money :amount="data.spent_today" :decimals="0" :currency="data?.currency" /></dd>
        <template v-if="data.manual_limit">
          <dt class="text-muted-foreground">{{ t('dailyBudget.manualLimit') }}</dt>
          <dd class="text-right"><Money :amount="data.manual_limit" :decimals="0" :currency="data?.currency" /></dd>
        </template>
      </dl>
    </div>
  </Card>
  <!-- Holds the cell while loading, so the widget does not pop in. -->
  <Card size="sm" v-else><Skeleton class="h-24 w-full" /></Card>
</template>
