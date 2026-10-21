<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Pencil, Plus, Trash2 } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { BudgetDialog } from '@/features/budget-edit'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ListHeader } from '@/shared/ui/list-header'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { drillTo, localDay } from '@/shared/lib/drill'
import { formatMoney } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { ProgressBar } from '@/shared/ui/progress-bar'

const { t } = useI18n()
const queryClient = useQueryClient()

const budgets = useQuery({ queryKey: ['budgets'], queryFn: () => call<Budget[]>('list_budgets') })
const statuses = useQuery({ queryKey: ['budget-status'], queryFn: () => call<BudgetStatus[]>('get_budget_status') })
const categories = useCategories()
const tree = useStoreTree()

function targetText(target: BudgetTarget): string {
  if (target === 'total') return t('budget.targetKind.total')
  if ('category' in target) return categoryKeyLabel(target.category, categories.data.value, t)
  const id = target.store.store_ids[0] ?? ''
  return flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === id)?.path.join(' › ') ?? id
}

/** Each budget with its status, matched once per list rather than per read. */
const rows = computed(() => {
  const byId = new Map((statuses.data.value ?? []).map((status) => [status.budget.id, status]))
  return (budgets.data.value ?? []).map((budget) => ({ budget, status: byId.get(budget.id) ?? null }))
})
/** The operations the spent figure is made of: the budget's period, category or wallets. */
function spentOf(status: BudgetStatus) {
  const target = status.budget.target
  return drillTo({
    from: localDay(status.period_start),
    to: localDay(status.period_end, true),
    kinds: ['payment'],
    category: typeof target === 'object' && 'category' in target ? target.category : undefined,
    storeIds: typeof target === 'object' && 'store' in target ? target.store.store_ids : undefined,
  })
}

const usedPct = (status: BudgetStatus | null) => Math.min(100, Math.round(Number(status?.used ?? 0) * 100))

const dialogOpen = ref(false)
const editing = ref<Budget | null>(null)
function openNew() {
  editing.value = null
  dialogOpen.value = true
}
function openEdit(budget: Budget) {
  editing.value = budget
  dialogOpen.value = true
}

const remove = useMutation({
  mutationFn: (id: string) => call('delete_budget', { id }),
  onSuccess: () => {
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
  },
  meta: { error: 'budget.deleteFailed', success: 'budget.deleted' },
})

const empty = computed(() => budgets.isSuccess.value && !budgets.data.value?.length)
</script>

<template>
  <div class="grid gap-4">
    <ListHeader :intro="t('budget.intro')" :action-label="t('budget.new')" :hide-action="empty" @action="openNew" />

    <EmptyState v-if="empty" compact :title="t('budget.empty')">
      <Button @click="openNew"><Plus class="size-4" />{{ t('budget.new') }}</Button>
    </EmptyState>

    <div class="grid gap-4 @wide:grid-cols-2">
      <Card v-for="{ budget, status } in rows" :key="budget.id">
        <div class="grid gap-3">
          <div class="flex items-start justify-between gap-2">
            <div class="grid min-w-0 gap-1">
              <div class="flex flex-wrap items-center gap-2">
                <p class="truncate font-medium">{{ targetText(budget.target) }}</p>
                <Badge variant="outline">{{ t(`budget.periodKind.${budget.period}`) }}</Badge>
                <Badge v-if="!budget.active" variant="outline">{{ t('budget.paused') }}</Badge>
              </div>
              <p class="text-sm tabular-nums text-muted-foreground">
                <RouterLink v-if="status" :to="spentOf(status)" class="hover:underline" data-tour="budget-spent">
                  <Money :amount="status.spent" :currency="budget.currency" />
                </RouterLink>
                <Money v-else amount="0" :currency="budget.currency" />
                /
                <Money :amount="budget.limit" :currency="budget.currency" />
              </p>
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <Button
                variant="ghost"
                size="icon-sm"
                class="text-muted-foreground"
                :title="t('budget.edit')"
                :aria-label="t('budget.edit')"
                @click="openEdit(budget)"
              >
                <Pencil class="size-4" />
              </Button>
              <ConfirmButton :label="t('budget.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="remove.mutate(budget.id)">
                <Trash2 class="size-4" />
              </ConfirmButton>
            </div>
          </div>
          <div v-if="budget.active" class="grid gap-1">
            <ProgressBar
              :value="usedPct(status)"
              :tone="Number(status?.remaining ?? 1) < 0 ? 'destructive' : 'primary'"
              :label="targetText(budget.target)"
            />
            <p
              class="text-xs tabular-nums"
              :class="Number(status?.remaining ?? 0) < 0 ? 'text-destructive' : 'text-muted-foreground'"
            >
              {{
                Number(status?.remaining ?? 0) < 0
                  ? t('budget.over', { amount: formatMoney(String(-Number(status?.remaining)), { currency: budget.currency }) })
                  : t('budget.left', { amount: formatMoney(status?.remaining ?? budget.limit, { currency: budget.currency }) })
              }}
            </p>
          </div>
        </div>
      </Card>
    </div>

    <BudgetDialog v-model:open="dialogOpen" :budget="editing" />
  </div>
</template>
