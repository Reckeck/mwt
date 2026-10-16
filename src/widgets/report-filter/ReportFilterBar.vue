<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { CurrencyPicker } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { FILTER_FIELD_CLASS, FilterBar } from '@/shared/ui/filter-bar'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { Checkbox } from '@/shared/ui/checkbox'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import type { ReportFilterBarProps } from './report-filter.types'

const props = defineProps<ReportFilterBarProps>()
const filter = defineModel<ReportFilter>({ required: true })
const emit = defineEmits<{ reset: [] }>()

const { t } = useI18n()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

/** Set filters among the folded ones: a currency, today's money. */
const moreActive = computed(() => [filter.value.currency, filter.value.real_terms].filter(Boolean).length)

const storeId = computed({
  get: () => filter.value.scope.store_ids[0] ?? '',
  set: (id: string) => {
    filter.value.scope = { store_ids: id ? [id] : [], include_children: true }
  },
})
/** Month pickers hold `''` for «not set»; the filter holds `null`. */
const monthField = (field: 'from' | 'to') =>
  computed({
    get: () => filter.value[field] ?? '',
    set: (value: string) => {
      filter.value[field] = value || null
    },
  })
const fromMonth = monthField('from')
const toMonth = monthField('to')

const year = computed({
  get: () => String(filter.value.year ?? new Date().getFullYear()),
  set: (value: string) => {
    filter.value.year = Number(value) || null
  },
})
const years = computed(() => {
  const now = new Date().getFullYear()
  return Array.from({ length: 8 }, (_unused, index) => String(now - index))
})
/** Wallet scope means nothing for inflation and the year of the budget. */
const scoped = computed(() => props.kind !== 'year' && props.kind !== 'inflation')
</script>

<template>
  <FilterBar :active="props.active" :more-active="moreActive" @reset="emit('reset')">
    <template v-if="props.kind === 'year'">
      <div :class="FILTER_FIELD_CLASS">
        <Label for="report-year">{{ t('report.filter.year') }}</Label>
        <Select id="report-year" v-model="year" class="w-full @3xl:w-28">
          <SelectItem v-for="entry in years" :key="entry" :value="entry">{{ entry }}</SelectItem>
        </Select>
      </div>
    </template>
    <template v-else>
      <div :class="FILTER_FIELD_CLASS">
        <Label for="report-from">{{ t('report.filter.from') }}</Label>
        <DateTimePicker id="report-from" v-model="fromMonth" mode="month" class="w-full @3xl:w-40" />
      </div>
      <div :class="FILTER_FIELD_CLASS">
        <Label for="report-to">{{ t('report.filter.to') }}</Label>
        <DateTimePicker id="report-to" v-model="toMonth" mode="month" class="w-full @3xl:w-40" />
      </div>
    </template>
    <div v-if="scoped" :class="FILTER_FIELD_CLASS">
      <Label for="report-scope">{{ t('report.filter.scope') }}</Label>
      <StoreNodeSelect id="report-scope" v-model="storeId" :nodes="nodes" :empty-label="t('report.filter.allWallets')" class="w-full @3xl:w-56" />
    </div>
    <!-- Rarely changed: folded under «Ещё фильтры». -->
    <template #more>
      <div class="grid gap-1.5">
        <Label for="report-currency">{{ t('report.filter.currency') }}</Label>
        <div>
          <CurrencyPicker
            id="report-currency"
            allow-empty
            :placeholder="t('report.filter.budgetCurrency')"
            :model-value="filter.currency ?? ''"
            @update:model-value="(value) => (filter.currency = String(value) || null)"
          />
        </div>
      </div>
      <label v-if="props.kind !== 'inflation'" for="report-real-terms" class="flex items-center gap-2 text-sm">
        <Checkbox id="report-real-terms" v-model="filter.real_terms" />
        {{ t('report.filter.realTerms') }}
      </label>
    </template>
  </FilterBar>
</template>
