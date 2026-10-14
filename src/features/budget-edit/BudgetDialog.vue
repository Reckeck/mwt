<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { CurrencyPicker } from '@/entities/currency'
import { CategorySelect } from '@/features/category-select'
import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'
import { Switch } from '@/shared/ui/switch'

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

const form = useFormValidation(
  () => ({
    category: targetKind.value === 'category' ? required(category.value, 'pick') : null,
    store: targetKind.value === 'store' ? required(storeId.value, 'pick') : null,
    limit: positiveAmount(limit.value),
    currency: required(currency.value, 'pick'),
  }),
  { resetOn: open },
)

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
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'budget.saveFailed', success: () => props.budget ? t('budget.saved') : t('budget.created') },
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.budget ? t('budget.edit') : t('budget.new')">
    <form id="budget-form" class="@container grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <div class="grid gap-3 @sm:grid-cols-2">
        <div class="grid gap-1.5">
          <Label for="budget-target">{{ t('budget.target') }}</Label>
          <Select id="budget-target" v-model="targetKind">
            <option v-for="kind in BUDGET_TARGETS" :key="kind" :value="kind">{{ t(`budget.targetKind.${kind}`) }}</option>
          </Select>
        </div>
        <FormField
          v-if="targetKind === 'category'"
          for="budget-category"
          :label="t('budget.category')"
          required
          :error="form.error('category')"
        >
          <CategorySelect
            id="budget-category"
            v-model="category"
            :groups="['daily', 'fixed']"
            :placeholder="t('budget.pickCategory')"
          />
        </FormField>
        <FormField
          v-else-if="targetKind === 'store'"
          for="budget-store"
          :label="t('budget.store')"
          required
          :error="form.error('store')"
        >
          <StoreNodeSelect id="budget-store" v-model="storeId" :nodes="liveNodes" :empty-label="t('budget.pickStore')" />
        </FormField>
      </div>

      <div class="grid gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label for="budget-period">{{ t('budget.period') }}</Label>
          <Select id="budget-period" v-model="period">
            <option v-for="entry in BUDGET_PERIODS" :key="entry" :value="entry">{{ t(`budget.periodKind.${entry}`) }}</option>
          </Select>
        </div>
        <FormField for="budget-limit" :label="t('budget.limit')" required :error="form.error('limit')">
          <AmountInput id="budget-limit" v-model="limit" />
        </FormField>
        <FormField for="budget-currency" :label="t('budget.currency')" required :error="form.error('currency')">
          <CurrencyPicker id="budget-currency" v-model="currency" />
        </FormField>
      </div>

      <div class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2">
        <Label for="budget-active">{{ t('budget.active') }}</Label>
        <Switch id="budget-active" v-model="active" />
      </div>
      <p class="text-xs text-muted-foreground">{{ t('budget.hint') }}</p>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="budget-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
