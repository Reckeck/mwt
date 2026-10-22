<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { CircleAlert } from 'lucide-vue-next'

import { buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { formatDate } from '@/shared/lib/format'
import { EntityLink } from '@/shared/ui/entity-link'
import { Money } from '@/entities/currency'
import { SectionState } from '@/shared/ui/section-state'
import { ProgressBar } from '@/shared/ui/progress-bar'

const { t } = useI18n()

const progress = useQuery({ queryKey: ['goal-progress'], queryFn: () => call<GoalProgress[]>('get_goal_progress') })
const pct = (value: string) => Math.min(100, Math.max(0, Number(value)))
</script>

<template>
  <Card size="sm" v-if="progress.data.value?.length">
    <template #header>
      <RouterLink data-widget-title :to="{ name: 'planning-goals' }" class="text-sm text-muted-foreground hover:text-foreground">
        {{ t('goal.dashboardTitle') }}
      </RouterLink>
    </template>
    <ul class="grid gap-3">
      <li v-for="item in progress.data.value" :key="item.goal.id" class="grid gap-1">
        <div class="flex items-center justify-between gap-2 text-sm">
          <span class="flex min-w-0 items-center gap-1.5 truncate">
            <CircleAlert v-if="!item.realistic" class="size-3.5 shrink-0 text-warning" :aria-label="t('goal.revise')" />
            <EntityLink :target="{ kind: 'goal', id: item.goal.id }" :label="item.goal.name" plain />
          </span>
          <span class="shrink-0 tabular-nums text-muted-foreground">{{ item.pct }}%</span>
        </div>
        <ProgressBar :value="pct(item.pct)" size="sm" />
        <p v-if="item.need_per_month" class="text-xs tabular-nums text-muted-foreground">
          {{ t('goal.needPerMonth') }}
          <Money :amount="item.need_per_month" :decimals="0" :currency="item.goal.currency" />
          <template v-if="item.forecast"> · {{ t('goal.forecastShort', { date: formatDate(item.forecast) }) }}</template>
        </p>
        <p v-if="item.planned_now" class="text-xs tabular-nums text-muted-foreground">
          {{ t('goal.planVsFact') }}
          <Money :amount="item.planned_now" :decimals="0" :currency="item.goal.currency" /> /
          <Money
            :amount="item.saved"
            :decimals="0"
            :currency="item.goal.currency"
            :class="Number(item.saved) < Number(item.planned_now) ? 'text-warning' : 'text-success'"
          />
        </p>
      </li>
    </ul>
  </Card>
  <Card size="sm" v-else-if="progress.isPending.value"><Skeleton class="h-24 w-full" /></Card>
  <!-- A failed load is not «nothing here»: it said «create one» to someone who has several. -->
  <Card size="sm" v-else-if="progress.error.value">
    <SectionState :error="progress.error.value" compact @retry="progress.refetch()" />
  </Card>
  <Card size="sm" v-else>
    <div class="flex flex-col items-start gap-2">
      <p class="text-sm text-muted-foreground">{{ t('dashboard.widgetEmpty') }}</p>
      <RouterLink :to="{ name: 'planning-goals' }" :class="buttonVariants({ variant: 'outline', size: 'sm' })">
        {{ t('goal.new') }}
      </RouterLink>
    </div>
  </Card>
</template>
