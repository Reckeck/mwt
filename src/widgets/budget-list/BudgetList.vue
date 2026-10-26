<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { useQuery } from '@tanstack/vue-query'
import { Pencil, Plus, Trash2 } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { BudgetDeleteDialog, BudgetDialog } from '@/features/budget-edit'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ListHeader } from '@/shared/ui/list-header'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { EntityLink } from '@/shared/ui/entity-link'
import { call } from '@/shared/api/ipc'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { useFocusRow } from '@/shared/lib/use-focus-row'
import { drillTo, localDay } from '@/shared/lib/drill'
import { budgetEntity } from '@/shared/lib/entity-link'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { ProgressBar } from '@/shared/ui/progress-bar'

const { t } = useI18n()

const budgets = useQuery({ queryKey: ['budgets'], queryFn: () => call<Budget[]>('list_budgets') })
const statuses = useQuery({ queryKey: ['budget-status'], queryFn: () => call<BudgetStatus[]>('get_budget_status') })
useFocusRow(() => budgets.isSuccess.value && statuses.isSuccess.value)
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

const deleting = ref<{ budget: Budget; name: string } | null>(null)
const deleteOpen = ref(false)
function askDelete(budget: Budget) {
  deleting.value = { budget, name: targetText(budget.target) }
  deleteOpen.value = true
}

const empty = computed(() => budgets.isSuccess.value && !budgets.data.value?.length)
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <ListHeader :intro="t('budget.intro')" :action-label="t('budget.new')" :hide-action="empty" @action="openNew" />

    <EmptyState v-if="empty" compact :title="t('budget.empty')">
      <Button @click="openNew"><Plus class="size-4" />{{ t('budget.new') }}</Button>
    </EmptyState>

    <div class="grid grid-cols-1 gap-4 @wide:grid-cols-2">
      <Card v-for="{ budget, status } in rows" :key="budget.id" :data-focus-row="budget.id">
        <div class="grid gap-3">
          <div class="flex items-start justify-between gap-2">
            <div class="grid min-w-0 gap-1">
              <div class="flex flex-wrap items-center gap-2">
                <p class="min-w-0 truncate font-medium">
                  <EntityLink :target="budgetEntity(budget)" :label="targetText(budget.target)" plain />
                </p>
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
            <div class="flex shrink-0 items-center gap-2">
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
              <Button
                variant="ghost"
                size="icon-sm"
                class="text-destructive"
                :title="t('budget.delete')"
                :aria-label="t('budget.delete')"
                @click="askDelete(budget)"
              >
                <Trash2 class="size-4" />
              </Button>
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
              <i18n-t v-if="Number(status?.remaining ?? 0) < 0" keypath="budget.over">
                <template #amount><Money :amount="String(-Number(status?.remaining))" :currency="budget.currency" /></template>
              </i18n-t>
              <i18n-t v-else keypath="budget.left">
                <template #amount><Money :amount="status?.remaining ?? budget.limit" :currency="budget.currency" /></template>
              </i18n-t>
            </p>
          </div>
        </div>
      </Card>
    </div>

    <BudgetDialog v-model:open="dialogOpen" :budget="editing" />
    <BudgetDeleteDialog v-model:open="deleteOpen" :budget="deleting?.budget ?? null" :name="deleting?.name ?? ''" />
  </div>
</template>
