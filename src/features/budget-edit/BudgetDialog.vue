<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { CategorySelect } from '@/features/category-select'
import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call, parseError } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

import { BUDGET_PERIODS, BUDGET_TARGETS, type BudgetTargetKind } from './budget-edit.constants'
import type { BudgetDialogProps } from './budget-edit.types'

const props = withDefaults(defineProps<BudgetDialogProps>(), { budget: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

/** Limits watch spending, so only spending groups are offered. */
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const liveNodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const targetKind = ref<BudgetTargetKind>('total')
const category = ref('')
const storeId = ref('')
const period = ref<BudgetPeriod>('month')
const limit = ref('')
const currency = ref('')
const active = ref(true)

watch(open, (isOpen) => {
  if (!isOpen) return
  const budget = props.budget
  const target = budget?.target ?? 'total'
  targetKind.value = target === 'total' ? 'total' : 'category' in target ? 'category' : 'store'
  category.value = typeof target === 'object' && 'category' in target ? target.category : ''
  storeId.value = typeof target === 'object' && 'store' in target ? (target.store.store_ids[0] ?? '') : ''
  period.value = budget?.period ?? 'month'
  limit.value = budget?.limit ?? ''
  currency.value = budget?.currency ?? (settings.vault?.budget_currency || settings.baseCurrency)
  active.value = budget?.active ?? true
})

const target = computed<BudgetTarget | null>(() => {
  if (targetKind.value === 'total') return 'total'
  if (targetKind.value === 'category') return category.value ? { category: category.value } : null
  return storeId.value ? { store: { store_ids: [storeId.value], include_children: true } } : null
})

const valid = computed(() => target.value !== null && Number(limit.value) > 0 && currency.value.trim() !== '')

const save = useMutation({
  mutationFn: () => {
    const input: BudgetInput = {
      active: active.value,
      target: target.value as BudgetTarget,
      period: period.value,
      limit: limit.value.trim(),
      currency: currency.value.trim().toLowerCase(),
    }
    return props.budget
      ? call<Budget>('update_budget', { input: { id: props.budget.id, budget: input } })
      : call<Budget>('create_budget', { input })
  },
  onSuccess: () => {
    toast.success(props.budget ? t('budget.saved') : t('budget.created'))
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  onError: (err) => toast.error(t('budget.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.budget ? t('budget.edit') : t('budget.new')">
    <form id="budget-form" class="@container grid gap-4" @submit.prevent="valid && save.mutate()">
      <div class="grid gap-3 @sm:grid-cols-2">
        <div class="grid gap-1.5">
          <Label for="budget-target">{{ t('budget.target') }}</Label>
          <Select id="budget-target" v-model="targetKind">
            <option v-for="kind in BUDGET_TARGETS" :key="kind" :value="kind">{{ t(`budget.targetKind.${kind}`) }}</option>
          </Select>
        </div>
        <div v-if="targetKind === 'category'" class="grid gap-1.5">
          <Label for="budget-category">{{ t('budget.category') }}</Label>
          <CategorySelect
            id="budget-category"
            v-model="category"
            :groups="['daily', 'fixed']"
            :placeholder="t('budget.pickCategory')"
          />
        </div>
        <div v-else-if="targetKind === 'store'" class="grid gap-1.5">
          <Label for="budget-store">{{ t('budget.store') }}</Label>
          <StoreNodeSelect id="budget-store" v-model="storeId" :nodes="liveNodes" :empty-label="t('budget.pickStore')" />
        </div>
      </div>

      <div class="grid gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label for="budget-period">{{ t('budget.period') }}</Label>
          <Select id="budget-period" v-model="period">
            <option v-for="entry in BUDGET_PERIODS" :key="entry" :value="entry">{{ t(`budget.periodKind.${entry}`) }}</option>
          </Select>
        </div>
        <div class="grid gap-1.5">
          <Label for="budget-limit">{{ t('budget.limit') }}</Label>
          <Input id="budget-limit" v-model="limit" inputmode="decimal" />
        </div>
        <div class="grid gap-1.5">
          <Label for="budget-currency">{{ t('budget.currency') }}</Label>
          <Select id="budget-currency" v-model="currency">
            <option v-for="code in settings.allCurrencies" :key="code" :value="code">{{ code.toUpperCase() }}</option>
          </Select>
        </div>
      </div>

      <label class="flex items-center gap-2 text-sm">
        <input v-model="active" type="checkbox" class="size-4 accent-primary" />
        {{ t('budget.active') }}
      </label>
      <p class="text-xs text-muted-foreground">{{ t('budget.hint') }}</p>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="budget-form" type="submit" :disabled="!valid || save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
