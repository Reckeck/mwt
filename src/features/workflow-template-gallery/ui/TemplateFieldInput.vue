<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { CurrencyPicker, Money } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryOptions } from '@/shared/lib/categories'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { AmountInput } from '@/shared/ui/amount-input'
import { Select, SelectItem } from '@/shared/ui/select'

/** One field of a template form, drawn with the app's own picker for its kind. */
const props = defineProps<{ field: TemplateField; templateId: string; id: string; invalid?: boolean }>()
const model = defineModel<string>({ required: true })
const { t } = useI18n()

const tree = useStoreTree()
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))
const budgets = useQuery({
  queryKey: ['budgets'],
  queryFn: () => call<Budget[]>('list_budgets'),
  enabled: computed(() => props.field.kind === 'budget'),
})
/** Income for `Pay yourself first`, spending for the watchers. */
const groups = computed<CategoryGroup[]>(() =>
  props.templateId === 'pay_yourself_first' ? ['income'] : ['fixed', 'daily'],
)
const categories = useCategories()
const categoryChoices = computed(() => categoryOptions(categories.data.value, t, groups.value))
const WEEKDAYS = [0, 1, 2, 3, 4, 5, 6]
const DAYS = Array.from({ length: 28 }, (_, index) => index + 1)
</script>

<template>
  <StoreNodeSelect
    v-if="props.field.kind === 'store'"
    :id="props.id"
    v-model="model"
    :nodes="nodes"
    :placeholder="t('workflowTemplates.pickStore')"
    :invalid="props.invalid"
  />
  <Select v-else-if="props.field.kind === 'category'" :id="props.id" v-model="model">
    <SelectItem v-for="choice in categoryChoices" :key="choice.value" :value="choice.value">{{ choice.label }}</SelectItem>
  </Select>
  <CurrencyPicker v-else-if="props.field.kind === 'currency'" :id="props.id" v-model="model" />
  <Select v-else-if="props.field.kind === 'budget'" :id="props.id" v-model="model" :placeholder="t('workflowTemplates.pickBudget')">
    <SelectItem v-for="budget in budgets.data.value ?? []" :key="budget.id" :value="budget.id">
      {{ t(`budget.periodKind.${budget.period}`) }} · <Money :amount="budget.limit" :currency="budget.currency" />
    </SelectItem>
  </Select>
  <Select v-else-if="props.field.kind === 'weekday'" :id="props.id" v-model="model">
    <SelectItem v-for="day in WEEKDAYS" :key="day" :value="String(day)">{{ t(`schedule.weekday.${day}`) }}</SelectItem>
  </Select>
  <Select v-else-if="props.field.kind === 'day'" :id="props.id" v-model="model">
    <SelectItem v-for="day in DAYS" :key="day" :value="String(day)">{{ day }}</SelectItem>
  </Select>
  <AmountInput v-else :id="props.id" v-model="model" />
</template>
