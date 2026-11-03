<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { CategorySelect } from '@/features/category-select'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { SyncChangedBanner } from '@/shared/ui/sync-changed-banner'
import { WizardSteps } from '@/shared/ui/wizard-steps'
import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { markFreshBudget } from '@/shared/lib/planning-fresh'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { positiveAmount, required, useEditBase, useFormValidation } from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'

import type { BudgetTargetKind } from './budget-edit.constants'
import type { BudgetDialogProps, BudgetField } from './budget-edit.types'
import { budgetInputOf, budgetInputOfFields, fieldsOfBudget, type BudgetFields } from './lib/budget-fields'
import BudgetDeleteDialog from './BudgetDeleteDialog.vue'
import BudgetAmountStep from './ui/BudgetAmountStep.vue'
import BudgetTargetStep from './ui/BudgetTargetStep.vue'

/**
 * A new limit takes two steps (Money 1 · 3–4): on what and per what period, then how
 * much. An existing one changes in one form (Full-New-Fixes 5), with `Delete…` in
 * its footer. Saving says what was set and offers to take it back (Money 1 · 5).
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
const deleteOpen = ref(false)
/** An existing limit changes on one page, both steps' fields together. */
const onePage = computed(() => props.budget !== null)
const targetKind = ref<BudgetTargetKind>('category')
const category = ref('')
const storeId = ref('')
const period = ref<BudgetPeriod>('month')
const limit = ref('')
const currency = ref('')
const warn = ref(true)
const active = ref(true)

/** Writes fields into the form; `Refresh` and opening both go through it. */
function load(fields: BudgetFields) {
  targetKind.value = fields.targetKind
  category.value = fields.category
  storeId.value = fields.storeId
  period.value = fields.period
  limit.value = fields.limit
  currency.value = fields.currency
  warn.value = fields.warn
  active.value = fields.active
}
const currentFields = (): BudgetFields => ({
  targetKind: targetKind.value,
  category: category.value,
  storeId: storeId.value,
  period: period.value,
  limit: limit.value,
  currency: currency.value,
  warn: warn.value,
  active: active.value,
})

watch(open, (isOpen) => {
  if (!isOpen) return
  step.value = 1
  load(
    fieldsOfBudget(
      props.budget ?? {
        active: true,
        target: { category: props.category },
        period: 'month',
        limit: '',
        currency: settings.vault?.budget_currency || settings.baseCurrency,
      },
    ),
  )
})

/** The limit as the sync keeps it now (`['budgets']` is re-read after `vault-synced`); the prop may be a stale copy. */
const budgets = useQuery({ queryKey: ['budgets'], queryFn: () => call<Budget[]>('list_budgets'), enabled: open })
const edit = useEditBase({
  open,
  entity: () => props.budget,
  live: () => (props.budget ? budgets.data.value?.find((entry) => entry.id === props.budget?.id) : undefined),
  model: (entity: Budget) => fieldsOfBudget(entity),
})
function refreshFromOtherDevice() {
  edit.refresh(currentFields, load)
}

const target = computed<BudgetTarget>(() => {
  if (targetKind.value === 'total') return 'total'
  if (targetKind.value === 'category') return { category: category.value }
  return { store: { store_ids: [storeId.value], include_children: true } }
})

/** Each step checks its own fields: `Next` the first, `Save` the second. */
const whatRules = () => ({
  category: targetKind.value === 'category' ? required(category.value, 'pick') : null,
  store: targetKind.value === 'store' ? required(storeId.value, 'pick') : null,
})
const amountRules = () => ({ limit: positiveAmount(limit.value), currency: required(currency.value, 'pick') })
const form = useFormValidation<BudgetField>(
  () => (onePage.value ? { ...whatRules(), ...amountRules() } : step.value === 1 ? whatRules() : amountRules()),
  { resetOn: [open, step] },
)

const title = computed(() => (props.budget ? t('budget.editOn', { name: currentName.value }) : t(`budget.newOn.${targetKind.value}`)))
const steps = computed(() => [t('budget.step.what'), t('budget.step.howMuch')])

/** `Groceries`, `all spending`, `Chase`: what the toast says the limit is on. */
function targetName(input: BudgetInput): string {
  if (input.target === 'total') return t('budget.allSpending')
  if ('category' in input.target) return categoryKeyLabel(input.target.category, categories.data.value, t)
  const id = input.target.store.store_ids[0]
  return liveNodes.value.find((node) => node.store.id === id)?.store.name ?? ''
}

const currentName = computed(() => (props.budget ? targetName({ ...props.budget, target: target.value }) : ''))

const save = useMutation({
  mutationFn: (input: BudgetInput) =>
    props.budget
      ? call<Budget>('update_budget', {
          input: { id: props.budget.id, budget: input, base: edit.base.value && budgetInputOf(edit.base.value) },
        })
      : call<Budget>('create_budget', { input }),
  onSuccess: (budget) => {
    if (!props.budget) markFreshBudget(budget.id)
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: {
    error: 'budget.saveFailed',
    // money: plain — the toast after a save is text
    success: (_data, input) =>
      props.budget
        ? t('budget.updatedOn', { target: targetName(input as BudgetInput) })
        : t('budget.savedOn', {
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
  if (step.value === 1 && !onePage.value) {
    form.submit(() => (step.value = 2), event)
    return
  }
  form.submit(
    () =>
      save.mutate(budgetInputOfFields(currentFields())),
    event,
  )
}
</script>

<template>
  <Dialog overlay="budget" v-model:open="open" :title="title">
    <form id="budget-form" class="grid gap-4" @submit.prevent="submit">
      <SyncChangedBanner v-if="edit.changedElsewhere.value" @refresh="refreshFromOtherDevice" />
      <WizardSteps v-if="!onePage" :step="step" :titles="steps" />
      <BudgetTargetStep
        v-if="step === 1 || onePage"
        v-model:kind="targetKind"
        v-model:store-id="storeId"
        v-model:period="period"
        :form="form"
        :nodes="liveNodes"
        :spending="!onePage && props.category && category === props.category ? props.spending : null"
      >
        <template #category>
          <CategorySelect id="budget-category" v-model="category" :groups="['daily', 'fixed']" :placeholder="t('budget.pickCategory')" />
        </template>
      </BudgetTargetStep>
      <BudgetAmountStep
        v-if="step === 2 || onePage"
        v-model:limit="limit"
        v-model:currency="currency"
        v-model:warn="warn"
        :form="form"
        :period="period"
        :spending="targetKind === 'category' && (onePage || category === props.category) ? props.spending : null"
        :focus="onePage"
      />
    </form>

    <template #footer>
      <Button v-if="onePage" variant="ghost" class="mr-auto text-destructive max-md:h-11" data-budget-delete @click="deleteOpen = true">
        {{ t('budget.deleteEllipsis') }}
      </Button>
      <Button variant="outline" class="max-md:h-11" @click="step === 1 || onePage ? (open = false) : (step = 1)">
        {{ step === 1 || onePage ? t('actions.cancel') : t('actions.back') }}
      </Button>
      <Button
        form="budget-form"
        type="submit"
        class="max-md:h-11 max-md:flex-1"
        :disabled="save.isPending.value"
        :data-wizard-next="(step === 1 && !onePage) || undefined"
        :data-wizard-save="step === 2 || onePage || undefined"
      >
        {{ step === 1 && !onePage ? t('actions.next') : t('actions.save') }}
      </Button>
    </template>
  </Dialog>
  <BudgetDeleteDialog v-model:open="deleteOpen" :budget="props.budget" :name="currentName" @deleted="open = false" />
</template>
