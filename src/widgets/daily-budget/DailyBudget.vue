<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney } from '@/shared/lib/format'

const { t } = useI18n()

const budget = useQuery({ queryKey: ['daily-budget'], queryFn: () => call<DailyBudget>('get_daily_budget') })
const data = computed(() => budget.data.value ?? null)
const money = (value: string | null | undefined) => formatMoney(value, { decimals: 0, currency: data.value?.currency })
/** Nothing to spread yet: no income, fact or plan, this month. */
const unset = computed(
  () => data.value !== null && data.value.mode === 'month' && Number(data.value.income) === 0,
)
</script>

<template>
  <Card v-if="data">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="text-sm text-muted-foreground">{{ t('dailyBudget.title') }}</p>
        <span class="text-xs text-muted-foreground">{{ t(`settings.dailyBudget.${data.mode}`) }}</span>
      </div>
    </template>

    <div v-if="unset" class="grid gap-2">
      <p class="text-sm text-muted-foreground">{{ t('dailyBudget.noIncome') }}</p>
      <RouterLink :to="{ name: 'planning-automations' }" :class="buttonVariants({ variant: 'outline', size: 'sm' })" class="w-fit">
        {{ t('dailyBudget.addSalary') }}
      </RouterLink>
    </div>
    <div v-else class="grid gap-3">
      <div>
        <p class="text-xs text-muted-foreground">{{ t('dailyBudget.leftToday') }}</p>
        <p
          class="text-3xl font-semibold tabular-nums"
          :class="Number(data.left_today) < 0 && 'text-destructive'"
        >
          {{ money(data.left_today) }}
        </p>
      </div>
      <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs tabular-nums">
        <dt class="text-muted-foreground">{{ t('dailyBudget.perDay') }}</dt>
        <dd class="text-right">{{ money(data.per_day) }}</dd>
        <template v-if="data.mode === 'month'">
          <dt class="text-muted-foreground">{{ t('dailyBudget.carry') }}</dt>
          <dd class="text-right" :class="Number(data.carry) < 0 ? 'text-destructive' : ''">{{ money(data.carry) }}</dd>
        </template>
        <template v-else>
          <dt class="text-muted-foreground">{{ t('dailyBudget.free') }}</dt>
          <dd class="text-right">{{ money(data.free) }}</dd>
          <dt class="text-muted-foreground">{{ t('dailyBudget.until') }}</dt>
          <dd class="text-right">{{ data.next_income ? formatDate(data.next_income) : t('dailyBudget.monthEnd') }}</dd>
        </template>
        <dt class="text-muted-foreground">{{ t('dailyBudget.spentToday') }}</dt>
        <dd class="text-right">{{ money(data.spent_today) }}</dd>
        <template v-if="data.manual_limit">
          <dt class="text-muted-foreground">{{ t('dailyBudget.manualLimit') }}</dt>
          <dd class="text-right">{{ money(data.manual_limit) }}</dd>
        </template>
      </dl>
    </div>
  </Card>
</template>
