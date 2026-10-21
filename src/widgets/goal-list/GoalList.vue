<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { CircleAlert, CircleCheck, Pencil, Plus, Star, Trash2 } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { GoalDialog } from '@/features/goal-edit'
import { Button } from '@/shared/ui/button'
import { ListHeader } from '@/shared/ui/list-header'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'

import GoalCalendarTable from './GoalCalendarTable.vue'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { EntityLink } from '@/shared/ui/entity-link'

const { t } = useI18n()
const queryClient = useQueryClient()

const goals = useQuery({ queryKey: ['goals'], queryFn: () => call<Goal[]>('list_goals') })
const progress = useQuery({ queryKey: ['goal-progress'], queryFn: () => call<GoalProgress[]>('get_goal_progress') })
const scenarios = useQuery({ queryKey: ['calc-scenarios'], queryFn: () => call<CalcScenario[]>('calc_scenarios_list') })
const scenarioName = (id: string) => scenarios.data.value?.find((entry) => entry.id === id)?.name ?? ''

/** Each goal with its progress, matched once (the card reads it a dozen times). */
const rows = computed(() => {
  const byId = new Map((progress.data.value ?? []).map((entry) => [entry.goal.id, entry]))
  return (goals.data.value ?? []).map((goal) => ({ goal, entry: byId.get(goal.id) ?? null }))
})

function issueText(issue: RealismIssue, currency: string): string {
  if ('over_commitment' in issue) {
    return t('goal.issue.commitment', {
      need: formatMoney(issue.over_commitment.need, { currency }),
      commitment: formatMoney(issue.over_commitment.commitment, { currency }),
    })
  }
  const data = issue.over_surplus
  return t('goal.issue.surplus', {
    need: formatMoney(data.need_total, { currency: data.currency }),
    surplus: formatMoney(data.surplus, { currency: data.currency }),
  })
}

const dialogOpen = ref(false)
const editing = ref<Goal | null>(null)
function openNew() {
  editing.value = null
  dialogOpen.value = true
}
function openEdit(goal: Goal) {
  editing.value = goal
  dialogOpen.value = true
}

const remove = useMutation({
  mutationFn: (id: string) => call('delete_goal', { id }),
  onSuccess: () => {
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
  },
  meta: { error: 'goal.deleteFailed', success: 'goal.deleted' },
})

const empty = computed(() => goals.isSuccess.value && !goals.data.value?.length)
const pct = (value: string | undefined) => Math.min(100, Math.max(0, Number(value ?? 0)))
</script>

<template>
  <div class="grid gap-4">
    <ListHeader :intro="t('goal.intro')" :action-label="t('goal.new')" :hide-action="empty" @action="openNew" />

    <EmptyState v-if="empty" compact :title="t('goal.empty')">
      <Button @click="openNew"><Plus class="size-4" />{{ t('goal.new') }}</Button>
    </EmptyState>

    <div class="grid gap-4 @wide:grid-cols-2">
      <Card v-for="{ goal, entry } in rows" :key="goal.id" :class="!goal.active && 'opacity-60'">
        <div class="grid gap-3">
          <div class="flex items-start justify-between gap-2">
            <div class="grid min-w-0 gap-1">
              <p class="truncate font-medium">{{ goal.name }}</p>
              <div class="flex items-center gap-0.5" :aria-label="t('goal.priorityValue', { value: goal.priority })">
                <Star
                  v-for="level in 5"
                  :key="level"
                  class="size-3.5"
                  :class="level <= goal.priority ? 'fill-warning text-warning' : 'text-muted-foreground/40'"
                />
              </div>
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <Button
                variant="ghost"
                size="icon-sm"
                class="text-muted-foreground"
                :title="t('goal.edit')"
                :aria-label="t('goal.edit')"
                @click="openEdit(goal)"
              >
                <Pencil class="size-4" />
              </Button>
              <ConfirmButton :label="t('goal.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="remove.mutate(goal.id)">
                <Trash2 class="size-4" />
              </ConfirmButton>
            </div>
          </div>

          <template v-if="entry">
            <div class="grid gap-1">
              <div class="flex items-baseline justify-between gap-2 text-sm tabular-nums">
                <span class="font-medium"><Money :amount="entry?.saved" :currency="goal.currency" /></span>
                <span class="text-muted-foreground">
                  <Money :amount="goal.target_amount" :currency="goal.currency" /> · {{ entry?.pct }}%
                </span>
              </div>
              <ProgressBar :value="pct(entry?.pct)" :label="goal.name" />
            </div>
            <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
              <dt class="text-muted-foreground">{{ t('goal.contributed') }}</dt>
              <dd class="text-right tabular-nums"><Money :amount="entry?.contributed" :currency="goal.currency" /></dd>
              <dt class="text-muted-foreground">{{ t('goal.fxGain') }}</dt>
              <dd class="text-right tabular-nums"><Money :amount="entry?.fx_gain" :currency="goal.currency" /></dd>
              <template v-if="entry?.need_per_month">
                <dt class="text-muted-foreground">{{ t('goal.needPerMonth') }}</dt>
                <dd class="text-right tabular-nums">
                  <Money :amount="entry?.need_per_month" :currency="goal.currency" />
                  · {{ t('goal.monthsLeft', { count: entry?.months_left ?? 0 }) }}
                </dd>
              </template>
              <dt class="text-muted-foreground">{{ t('goal.forecast') }}</dt>
              <dd class="text-right">
                {{ entry?.forecast ? formatDate(entry?.forecast) : t('goal.notAccumulating') }}
              </dd>
              <template v-if="goal.scenario_id && entry?.planned_now">
                <dt class="text-muted-foreground">{{ t('goal.plannedNow') }}</dt>
                <dd class="text-right tabular-nums">
                  <Money :amount="entry.planned_now" :currency="goal.currency" />
                  · {{ t('goal.actualNow') }} <Money :amount="entry.saved" :currency="goal.currency" />
                </dd>
                <dt class="text-muted-foreground">{{ t('goal.scenario') }}</dt>
                <dd class="justify-self-end">
                  <EntityLink :target="{ kind: 'scenario', id: goal.scenario_id }" :label="scenarioName(goal.scenario_id)" />
                </dd>
              </template>
            </dl>
            <p
              v-if="entry?.realistic"
              class="flex items-center gap-1.5 text-xs text-success"
            >
              <CircleCheck class="size-3.5" />
              {{ t('goal.realistic') }}
            </p>
            <div v-else class="grid gap-1 text-xs text-warning">
              <p class="flex items-center gap-1.5 font-medium">
                <CircleAlert class="size-3.5" />
                {{ t('goal.revise') }}
              </p>
              <p v-for="(issue, index) in entry?.issues ?? []" :key="index">
                {{ issueText(issue, goal.currency) }}
              </p>
            </div>
          </template>
        </div>
      </Card>
    </div>

    <GoalCalendarTable v-if="!empty" />

    <GoalDialog v-model:open="dialogOpen" :goal="editing" />
  </div>
</template>
