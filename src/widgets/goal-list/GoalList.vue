<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { CircleAlert, CircleCheck, Pencil, Plus, Star, Trash2 } from 'lucide-vue-next'

import { GoalDialog } from '@/features/goal-edit'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'

import GoalCalendarTable from './GoalCalendarTable.vue'

const { t } = useI18n()
const queryClient = useQueryClient()

const goals = useQuery({ queryKey: ['goals'], queryFn: () => call<Goal[]>('list_goals') })
const progress = useQuery({ queryKey: ['goal-progress'], queryFn: () => call<GoalProgress[]>('get_goal_progress') })

const progressOf = (id: string) => progress.data.value?.find((entry) => entry.goal.id === id) ?? null

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
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('goal.intro') }}</p>
      <Button @click="openNew">
        <Plus class="size-4" />
        {{ t('goal.new') }}
      </Button>
    </div>

    <EmptyState v-if="empty" compact :title="t('goal.empty')">
      <Button @click="openNew"><Plus class="size-4" />{{ t('goal.new') }}</Button>
    </EmptyState>

    <div class="grid gap-4 @lg:grid-cols-2">
      <Card v-for="goal in goals.data.value ?? []" :key="goal.id" :class="!goal.active && 'opacity-60'">
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
                size="icon"
                class="size-8 text-muted-foreground"
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

          <template v-if="progressOf(goal.id)">
            <div class="grid gap-1">
              <div class="flex items-baseline justify-between gap-2 text-sm tabular-nums">
                <span class="font-medium">{{ formatMoney(progressOf(goal.id)?.saved, { currency: goal.currency }) }}</span>
                <span class="text-muted-foreground">
                  {{ formatMoney(goal.target_amount, { currency: goal.currency }) }} · {{ progressOf(goal.id)?.pct }}%
                </span>
              </div>
              <div class="h-2 overflow-hidden rounded-full bg-muted" role="progressbar" :aria-valuenow="pct(progressOf(goal.id)?.pct)" aria-valuemin="0" aria-valuemax="100">
                <div class="h-full rounded-full bg-primary" :style="{ width: `${pct(progressOf(goal.id)?.pct)}%` }" />
              </div>
            </div>
            <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
              <dt class="text-muted-foreground">{{ t('goal.contributed') }}</dt>
              <dd class="text-right tabular-nums">{{ formatMoney(progressOf(goal.id)?.contributed, { currency: goal.currency }) }}</dd>
              <dt class="text-muted-foreground">{{ t('goal.fxGain') }}</dt>
              <dd class="text-right tabular-nums">{{ formatMoney(progressOf(goal.id)?.fx_gain, { currency: goal.currency }) }}</dd>
              <template v-if="progressOf(goal.id)?.need_per_month">
                <dt class="text-muted-foreground">{{ t('goal.needPerMonth') }}</dt>
                <dd class="text-right tabular-nums">
                  {{ formatMoney(progressOf(goal.id)?.need_per_month, { currency: goal.currency }) }}
                  · {{ t('goal.monthsLeft', { count: progressOf(goal.id)?.months_left ?? 0 }) }}
                </dd>
              </template>
              <dt class="text-muted-foreground">{{ t('goal.forecast') }}</dt>
              <dd class="text-right">
                {{ progressOf(goal.id)?.forecast ? formatDate(progressOf(goal.id)?.forecast) : t('goal.notAccumulating') }}
              </dd>
            </dl>
            <p
              v-if="progressOf(goal.id)?.realistic"
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
              <p v-for="(issue, index) in progressOf(goal.id)?.issues ?? []" :key="index">
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
