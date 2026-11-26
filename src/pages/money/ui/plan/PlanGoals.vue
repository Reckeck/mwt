<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'

import { GoalDialog } from '@/features/goal-edit'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { formatDate } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { SectionState } from '@/shared/ui/section-state'

/**
 * The plan's goals: saved of the target and when it comes at this pace. A row opens the goal's page
 * (top up, forecast, history, pause, delete); the full list with the calendar is `GoalList` below.
 */
const { t } = useI18n()
const progress = useQuery({ queryKey: ['goal-progress'], queryFn: () => call<GoalProgress[]>('get_goal_progress') })

const rows = computed(() => (progress.data.value ?? []).filter((entry) => entry.goal.active))
const pct = (value: string) => Math.min(100, Math.max(0, Number(value)))
const empty = computed(() => progress.isSuccess.value && !rows.value.length)

const createOpen = ref(false)
const emit = defineEmits<{ open: [goalId: string] }>()
</script>

<template>
  <section class="grid content-start gap-3 rounded-lg border border-border bg-card p-4" data-tour="plan-goals">
    <header class="flex items-center justify-between gap-2">
      <h2 class="heading-section">{{ t('money.plan.goals') }}</h2>
      <Button variant="outline" size="sm" data-plan-goal @click="createOpen = true"><Plus />{{ t('money.plan.goal') }}</Button>
    </header>
    <SectionState :loading="progress.isPending.value" :error="progress.error.value" skeleton="list" compact @retry="progress.refetch()">
      <EmptyState v-if="empty" compact :title="t('money.plan.noGoals')">
        <Button size="sm" @click="createOpen = true">{{ t('money.plan.createGoal') }}</Button>
      </EmptyState>
      <ul v-else class="grid gap-1">
        <li v-for="entry in rows" :key="entry.goal.id">
          <Button
            variant="ghost"
            class="grid h-auto w-full justify-items-stretch gap-1.5 px-2 py-2 text-left font-normal whitespace-normal max-md:min-h-11"
            data-goal-row
            @click="emit('open', entry.goal.id)">
            <span class="flex items-baseline justify-between gap-2 text-sm">
              <span class="min-w-0 truncate font-medium">{{ entry.goal.name }}</span>
              <span class="shrink-0 tabular-nums">
                <!-- The target's mark names the currency for both, so the saved sum shows the number only. -->
                <Money bare :amount="entry.saved" :currency="entry.goal.currency" :decimals="0" />
                /
                <Money :amount="entry.goal.target_amount" :currency="entry.goal.currency" :decimals="0" />
              </span>
            </span>
            <ProgressBar :value="pct(entry.pct)" size="sm" :label="entry.goal.name" />
            <span v-if="entry.forecast || entry.need_per_month" class="text-xs text-muted-foreground">
              <i18n-t v-if="entry.need_per_month" keypath="goal.needShort" tag="span">
                <template #amount><Money :amount="entry.need_per_month" :currency="entry.goal.currency" :decimals="0" /></template>
              </i18n-t>
              <template v-if="entry.need_per_month && entry.forecast"> · </template>
              <template v-if="entry.forecast">{{ t('goal.forecastShort', { date: formatDate(entry.forecast) }) }}</template>
            </span>
          </Button>
        </li>
      </ul>
    </SectionState>
    <GoalDialog v-model:open="createOpen" />
  </section>
</template>
