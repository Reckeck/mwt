<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Pencil, Plus, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { BudgetDialog } from '@/features/budget-edit'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call, parseError } from '@/shared/api/ipc'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'

const { t } = useI18n()
const queryClient = useQueryClient()

const budgets = useQuery({ queryKey: ['budgets'], queryFn: () => call<Budget[]>('list_budgets') })
const statuses = useQuery({ queryKey: ['budget-status'], queryFn: () => call<BudgetStatus[]>('get_budget_status') })
const categories = useQuery({ queryKey: ['categories'], queryFn: () => call<Category[]>('list_categories') })
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })

function targetText(target: BudgetTarget): string {
  if (target === 'total') return t('budget.targetKind.total')
  if ('category' in target) return categoryKeyLabel(target.category, categories.data.value, t)
  const id = target.store.store_ids[0] ?? ''
  return flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === id)?.path.join(' › ') ?? id
}

const statusOf = (id: string) => statuses.data.value?.find((status) => status.budget.id === id) ?? null
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
    toast.success(t('budget.deleted'))
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
  },
  onError: (err) => toast.error(t('budget.deleteFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const empty = computed(() => budgets.isSuccess.value && !budgets.data.value?.length)
</script>

<template>
  <div class="grid gap-4">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('budget.intro') }}</p>
      <Button @click="openNew">
        <Plus class="size-4" />
        {{ t('budget.new') }}
      </Button>
    </div>

    <div v-if="empty" class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
      {{ t('budget.empty') }}
    </div>

    <div class="grid gap-4 @lg:grid-cols-2">
      <Card v-for="budget in budgets.data.value ?? []" :key="budget.id">
        <div class="grid gap-3">
          <div class="flex items-start justify-between gap-2">
            <div class="grid min-w-0 gap-1">
              <div class="flex flex-wrap items-center gap-2">
                <p class="truncate font-medium">{{ targetText(budget.target) }}</p>
                <Badge variant="outline">{{ t(`budget.periodKind.${budget.period}`) }}</Badge>
                <Badge v-if="!budget.active" variant="outline">{{ t('budget.paused') }}</Badge>
              </div>
              <p class="text-sm tabular-nums text-muted-foreground">
                {{ formatMoney(statusOf(budget.id)?.spent ?? '0', { currency: budget.currency }) }}
                /
                {{ formatMoney(budget.limit, { currency: budget.currency }) }}
              </p>
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <Button
                variant="ghost"
                size="icon"
                class="size-8 text-muted-foreground"
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
            <div class="h-2 overflow-hidden rounded-full bg-muted" role="progressbar" :aria-valuenow="usedPct(statusOf(budget.id))" aria-valuemin="0" aria-valuemax="100">
              <div
                class="h-full rounded-full transition-all"
                :class="Number(statusOf(budget.id)?.remaining ?? 1) < 0 ? 'bg-destructive' : 'bg-primary'"
                :style="{ width: `${usedPct(statusOf(budget.id))}%` }"
              />
            </div>
            <p
              class="text-xs tabular-nums"
              :class="Number(statusOf(budget.id)?.remaining ?? 0) < 0 ? 'text-destructive' : 'text-muted-foreground'"
            >
              {{
                Number(statusOf(budget.id)?.remaining ?? 0) < 0
                  ? t('budget.over', { amount: formatMoney(String(-Number(statusOf(budget.id)?.remaining)), { currency: budget.currency }) })
                  : t('budget.left', { amount: formatMoney(statusOf(budget.id)?.remaining ?? budget.limit, { currency: budget.currency }) })
              }}
            </p>
          </div>
        </div>
      </Card>
    </div>

    <BudgetDialog v-model:open="dialogOpen" :budget="editing" />
  </div>
</template>
