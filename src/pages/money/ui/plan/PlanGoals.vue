<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'

import { GoalDialog, type GoalRemainder } from '@/features/goal-edit'
import { useReport } from '@/widgets/analytics-reports'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { formatDate } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { EntityLink } from '@/shared/ui/entity-link'
import { ProgressBar } from '@/shared/ui/progress-bar'

/**
 * The plan's goals (Money 2 · 9, 10): saved of the target and when it comes at
 * this pace. Without any, the month's remainder is offered as the first one.
 */
const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
const progress = useQuery({ queryKey: ['goal-progress'], queryFn: () => call<GoalProgress[]>('get_goal_progress') })
const where = useReport('where', () => props.filter)

const rows = computed(() => (progress.data.value ?? []).filter((entry) => entry.goal.active))
const pct = (value: string) => Math.min(100, Math.max(0, Number(value)))

/** What is left of the period's income, for a first goal. */
const remainder = computed<GoalRemainder | null>(() => {
  const data = where.data.value && 'where' in where.data.value ? where.data.value.where : null
  if (!data || !(Number(data.left) > 0)) return null
  return { amount: data.left, currency: data.meta.currency, month: (props.filter.from ?? new Date().toISOString()).slice(0, 7) }
})
const empty = computed(() => progress.isSuccess.value && !rows.value.length)

const open = ref(false)
</script>

<template>
  <section class="grid content-start gap-3 rounded-xl border bg-card p-4 shadow-sm" data-tour="plan-goals">
    <header class="flex items-center justify-between gap-2">
      <h2 class="heading-section">{{ t('money.plan.goals') }}</h2>
      <Button variant="outline" size="sm" data-plan-goal @click="open = true"><Plus />{{ t('money.plan.goal') }}</Button>
    </header>
    <ul v-if="rows.length" class="grid gap-3">
      <li v-for="entry in rows" :key="entry.goal.id" class="grid gap-1">
        <div class="flex items-baseline justify-between gap-2 text-sm">
          <EntityLink :target="{ kind: 'goal', id: entry.goal.id }" :label="entry.goal.name" plain class="min-w-0 truncate" />
          <span class="shrink-0 tabular-nums"><!-- The target's mark names the currency for both, so the saved sum shows the number only. -->
            <Money bare :amount="entry.saved" :currency="entry.goal.currency" :decimals="0" />
            /
            <Money :amount="entry.goal.target_amount" :currency="entry.goal.currency" :decimals="0" />
          </span>
        </div>
        <ProgressBar :value="pct(entry.pct)" size="sm" />
        <p v-if="entry.forecast || entry.need_per_month" class="text-xs text-muted-foreground">
          <i18n-t v-if="entry.need_per_month" keypath="goal.needShort">
            <template #amount><Money :amount="entry.need_per_month" :currency="entry.goal.currency" :decimals="0" /></template>
          </i18n-t>
          <template v-if="entry.need_per_month && entry.forecast"> · </template>
          <template v-if="entry.forecast">{{ t('goal.forecastShort', { date: formatDate(entry.forecast) }) }}</template>
        </p>
      </li>
    </ul>
    <div v-else-if="empty" class="grid justify-items-start gap-2 text-sm">
      <p>{{ t('money.plan.noGoals') }}</p>
      <p v-if="remainder" class="text-muted-foreground">
        <i18n-t keypath="money.plan.noGoalsHint">
          <template #month>{{ t(`period.of.${remainder.month.slice(5, 7)}`) }}</template>
          <template #amount><Money :amount="remainder.amount" :currency="remainder.currency" :decimals="0" /></template>
        </i18n-t>
      </p>
      <Button size="sm" class="max-md:h-10" @click="open = true">{{ t('money.plan.createGoal') }}</Button>
    </div>
    <GoalDialog v-model:open="open" :remainder="remainder" />
  </section>
</template>
