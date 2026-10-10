<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { CircleAlert } from 'lucide-vue-next'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'

const { t } = useI18n()

const progress = useQuery({ queryKey: ['goal-progress'], queryFn: () => call<GoalProgress[]>('get_goal_progress') })
const pct = (value: string) => Math.min(100, Math.max(0, Number(value)))
</script>

<template>
  <Card v-if="progress.data.value?.length">
    <template #header>
      <RouterLink :to="{ name: 'planning-goals' }" class="text-sm text-muted-foreground hover:text-foreground">
        {{ t('goal.dashboardTitle') }}
      </RouterLink>
    </template>
    <ul class="grid gap-3">
      <li v-for="item in progress.data.value" :key="item.goal.id" class="grid gap-1">
        <div class="flex items-center justify-between gap-2 text-sm">
          <span class="flex min-w-0 items-center gap-1.5 truncate">
            <CircleAlert v-if="!item.realistic" class="size-3.5 shrink-0 text-amber-500" :aria-label="t('goal.revise')" />
            {{ item.goal.name }}
          </span>
          <span class="shrink-0 tabular-nums text-muted-foreground">{{ item.pct }}%</span>
        </div>
        <div class="h-1.5 overflow-hidden rounded-full bg-muted">
          <div class="h-full rounded-full bg-primary" :style="{ width: `${pct(item.pct)}%` }" />
        </div>
        <p v-if="item.need_per_month" class="text-xs tabular-nums text-muted-foreground">
          {{ t('goal.needShort', { amount: formatMoney(item.need_per_month, { decimals: 0, currency: item.goal.currency }) }) }}
        </p>
      </li>
    </ul>
  </Card>
</template>
