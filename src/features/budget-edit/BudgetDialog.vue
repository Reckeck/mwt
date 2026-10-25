<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { CategorySelect } from '@/features/category-select'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { WizardSteps } from '@/shared/ui/wizard-steps'
import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'

import type { BudgetTargetKind } from './budget-edit.constants'
import type { BudgetDialogProps, BudgetField } from './budget-edit.types'
import BudgetAmountStep from './ui/BudgetAmountStep.vue'
import BudgetTargetStep from './ui/BudgetTargetStep.vue'

/**
 * A limit in two steps (Money 1 · 3–4): on what and per what period, then how
 * much. Saving says what was set and offers to take it back (Money 1 · 5).
 */
const props = withDefaults(defineProps<BudgetDialogProps>(), { budget: null, category: '', spending: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()
const categories = useCategories()

/** Limits watch spending, so only spending groups are offered. */
const tree = useStoreTree()
const liveNodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const step = ref(1)
const targetKind = ref<BudgetTargetKind>('category')
const category = ref('')
const storeId = ref('')
const period = ref<BudgetPeriod>('month')
const limit = ref('')
const currency = ref('')
const warn = ref(true)
const active = ref(true)

watch(open, (isOpen) => {
  if (!isOpen) return
  const budget = props.budget
  const target = budget?.target ?? (props.category ? { category: props.category } : { category: '' })
  step.value = 1
  targetKind.value = target === 'total' ? 'total' : 'category' in target ? 'category' : 'store'
  category.value = typeof target === 'object' && 'category' in target ? target.category : ''
  storeId.value = typeof target === 'object' && 'store' in target ? (target.store.store_ids[0] ?? '') : ''
  period.value = budget?.period ?? 'month'
  limit.value = budget?.limit ?? ''
  currency.value = budget?.currency ?? (settings.vault?.budget_currency || settings.baseCurrency)
  warn.value = budget?.warn ?? true
  active.value = budget?.active ?? true
})

const target = computed<BudgetTarget>(() => {
  if (targetKind.value === 'total') return 'total'
  if (targetKind.value === 'category') return { category: category.value }
  return { store: { store_ids: [storeId.value], include_children: true } }
})

/** Each step checks its own fields: «Далее» the first, «Сохранить» the second. */
const form = useFormValidation<BudgetField>(
  () =>
    step.value === 1
      ? {
          category: targetKind.value === 'category' ? required(category.value, 'pick') : null,
          store: targetKind.value === 'store' ? required(storeId.value, 'pick') : null,
        }
      : { limit: positiveAmount(limit.value), currency: required(currency.value, 'pick') },
  { resetOn: [open, step] },
)

const title = computed(() => (props.budget ? t('budget.edit') : t(`budget.newOn.${targetKind.value}`)))
const steps = computed(() => [t('budget.step.what'), t('budget.step.howMuch')])

/** «Продукты», «все траты», «Тинькофф»: what the toast says the limit is on. */
function targetName(input: BudgetInput): string {
  if (input.target === 'total') return t('budget.allSpending')
  if ('category' in input.target) return categoryKeyLabel(input.target.category, categories.data.value, t)
  const id = input.target.store.store_ids[0]
  return liveNodes.value.find((node) => node.store.id === id)?.store.name ?? ''
}

const save = useMutation({
  mutationFn: (input: BudgetInput) =>
    props.budget
      ? call<Budget>('update_budget', { input: { id: props.budget.id, budget: input } })
      : call<Budget>('create_budget', { input }),
  onSuccess: () => {
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: {
    error: 'budget.saveFailed',
    // money: plain — the toast after a save is text
    success: (_data, input) =>
      t('budget.savedOn', {
        target: targetName(input as BudgetInput),
        limit: formatMoney((input as BudgetInput).limit, { decimals: 0, currency: (input as BudgetInput).currency }),
        period: t(`budget.per.${(input as BudgetInput).period}`),
      }),
    undo: (data) => {
      const saved = data as Budget
      const before = props.budget
      if (!before) return call('delete_budget', { id: saved.id })
      const { id, ...budget } = before
      return call('update_budget', { input: { id, budget } })
    },
  },
})

function submit(event: Event) {
  if (step.value === 1) {
    form.submit(() => (step.value = 2), event)
    return
  }
  form.submit(
    () =>
      save.mutate({
        active: active.value,
        target: target.value,
        period: period.value,
        limit: limit.value.trim(),
        currency: currency.value.trim().toLowerCase(),
        warn: warn.value,
      }),
    event,
  )
}
</script>

<template>
  <Dialog overlay="budget" v-model:open="open" :title="title">
    <form id="budget-form" class="grid gap-4" @submit.prevent="submit">
      <WizardSteps :step="step" :titles="steps" />
      <BudgetTargetStep
        v-if="step === 1"
        v-model:kind="targetKind"
        v-model:store-id="storeId"
        v-model:period="period"
        :form="form"
        :nodes="liveNodes"
        :spending="props.category && category === props.category ? props.spending : null"
      >
        <template #category>
          <CategorySelect id="budget-category" v-model="category" :groups="['daily', 'fixed']" :placeholder="t('budget.pickCategory')" />
        </template>
      </BudgetTargetStep>
      <BudgetAmountStep
        v-else
        v-model:limit="limit"
        v-model:currency="currency"
        v-model:warn="warn"
        :form="form"
        :period="period"
        :spending="targetKind === 'category' && category === props.category ? props.spending : null"
      />
    </form>

    <template #footer>
      <Button variant="outline" class="max-md:h-11" @click="step === 1 ? (open = false) : (step = 1)">
        {{ step === 1 ? t('actions.cancel') : t('actions.back') }}
      </Button>
      <Button
        form="budget-form"
        type="submit"
        class="max-md:h-11 max-md:flex-1"
        :disabled="save.isPending.value"
        :data-wizard-next="step === 1 || undefined"
        :data-wizard-save="step === 2 || undefined"
      >
        {{ step === 1 ? t('actions.next') : t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
