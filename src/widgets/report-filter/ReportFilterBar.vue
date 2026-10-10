<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { StoreNodeSelect } from '@/entities/store'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

import type { ReportFilterBarProps } from './report-filter.types'

const props = defineProps<ReportFilterBarProps>()
const filter = defineModel<ReportFilter>({ required: true })

const { t } = useI18n()
const settings = useSettingsStore()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const storeId = computed({
  get: () => filter.value.scope.store_ids[0] ?? '',
  set: (id: string) => {
    filter.value.scope = { store_ids: id ? [id] : [], include_children: true }
  },
})
/** `type="month"` inputs hold `''` for «not set»; the filter holds `null`. */
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
  <div class="flex flex-wrap items-end gap-3">
    <template v-if="props.kind === 'year'">
      <div class="grid gap-1.5">
        <Label for="report-year">{{ t('report.filter.year') }}</Label>
        <Select id="report-year" v-model="year" class="w-28">
          <option v-for="entry in years" :key="entry" :value="entry">{{ entry }}</option>
        </Select>
      </div>
    </template>
    <template v-else>
      <div class="grid gap-1.5">
        <Label for="report-from">{{ t('report.filter.from') }}</Label>
        <Input id="report-from" v-model="fromMonth" type="month" class="w-40" />
      </div>
      <div class="grid gap-1.5">
        <Label for="report-to">{{ t('report.filter.to') }}</Label>
        <Input id="report-to" v-model="toMonth" type="month" class="w-40" />
      </div>
    </template>
    <div v-if="scoped" class="grid gap-1.5">
      <Label for="report-scope">{{ t('report.filter.scope') }}</Label>
      <StoreNodeSelect id="report-scope" v-model="storeId" :nodes="nodes" :empty-label="t('report.filter.allWallets')" class="w-56" />
    </div>
    <div class="grid gap-1.5">
      <Label for="report-currency">{{ t('report.filter.currency') }}</Label>
      <Select
        id="report-currency"
        class="w-44"
        :model-value="filter.currency ?? ''"
        @update:model-value="(value) => (filter.currency = String(value) || null)"
      >
        <option value="">{{ t('report.filter.budgetCurrency') }}</option>
        <option v-for="code in settings.allCurrencies" :key="code" :value="code">{{ code.toUpperCase() }}</option>
      </Select>
    </div>
    <label v-if="props.kind !== 'inflation'" class="flex h-9 items-center gap-2 text-sm">
      <input v-model="filter.real_terms" type="checkbox" class="size-4 accent-primary" />
      {{ t('report.filter.realTerms') }}
    </label>
  </div>
</template>
