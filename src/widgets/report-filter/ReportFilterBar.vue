<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { FILTER_FIELD_CLASS, FilterBar } from '@/shared/ui/filter-bar'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { Switch } from '@/shared/ui/switch'

import { PERIOD_PRESETS, defaultRange, presetOf, presetRange } from './model/period-presets'
import type { ReportFilterBarProps } from './report-filter.types'
import { formatMonthKey } from '@/shared/lib/format'
import { useStoreTree } from '@/shared/api/queries'

const props = withDefaults(defineProps<ReportFilterBarProps>(), { realTerms: false, wallets: true })
const filter = defineModel<ReportFilter>({ required: true })
/** The overview's month: its own value, so it does not move the other sections' period. */
const month = defineModel<string | null>('month', { default: null })
const emit = defineEmits<{ reset: [] }>()

const { t } = useI18n()
const route = useRoute()

const tree = useStoreTree()
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

/** Set filters among the folded ones: the currency. */
const moreActive = computed(() => (filter.value.currency ? 1 : 0))

const storeIds = computed({
  get: () => filter.value.scope.store_ids,
  set: (ids: string[]) => {
    filter.value.scope = { store_ids: ids, include_children: true }
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
const overviewMonth = computed({
  get: () => month.value ?? '',
  set: (value: string) => {
    month.value = value || null
  },
})

/** An empty picker shows the month the report really starts or ends with. */
const monthText = (key: string) => formatMonthKey(key, { month: 'short', year: 'numeric' })
const defaults = computed(() => defaultRange())

const preset = computed({
  get: () => presetOf(filter.value.from, filter.value.to),
  set: (value: string) => {
    if (!value) return
    Object.assign(filter.value, presetRange(value as (typeof PERIOD_PRESETS)[number]))
  },
})

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
</script>

<template>
  <FilterBar :active="props.active" :more-active="moreActive" :folds="props.wallets" @reset="emit('reset')">
    <template #lead>
      <div v-if="props.period === 'year'" :class="FILTER_FIELD_CLASS">
        <Label for="report-year">{{ t('report.filter.year') }}</Label>
        <Select id="report-year" v-model="year" class="w-full @3xl:w-28">
          <SelectItem v-for="entry in years" :key="entry" :value="entry">{{ entry }}</SelectItem>
        </Select>
      </div>
      <div v-else-if="props.period === 'month'" :class="FILTER_FIELD_CLASS">
        <Label for="report-month">{{ t('report.filter.month') }}</Label>
        <DateTimePicker
          id="report-month"
          v-model="overviewMonth"
          mode="month"
          :placeholder="monthText(defaults.to)"
          class="w-full @3xl:w-40"
        />
      </div>
      <div v-else :class="FILTER_FIELD_CLASS">
        <Label for="report-preset">{{ t('report.filter.period') }}</Label>
        <Select id="report-preset" v-model="preset" :placeholder="t('report.preset.custom')" class="w-full @3xl:w-44">
          <SelectItem v-for="entry in PERIOD_PRESETS" :key="entry" :value="entry">{{ t(`report.preset.${entry}`) }}</SelectItem>
        </Select>
      </div>
    </template>
    <template v-if="props.period === 'range'">
      <div :class="FILTER_FIELD_CLASS">
        <Label for="report-from">{{ t('report.filter.from') }}</Label>
        <DateTimePicker
          id="report-from"
          v-model="fromMonth"
          mode="month"
          :placeholder="monthText(defaults.from)"
          class="w-full @3xl:w-40"
        />
      </div>
      <div :class="FILTER_FIELD_CLASS">
        <Label for="report-to">{{ t('report.filter.to') }}</Label>
        <DateTimePicker
          id="report-to"
          v-model="toMonth"
          mode="month"
          :placeholder="monthText(defaults.to)"
          class="w-full @3xl:w-40"
        />
      </div>
    </template>
    <div v-if="props.wallets" :class="FILTER_FIELD_CLASS">
      <Label for="report-scope">{{ t('report.filter.scope') }}</Label>
      <StoreNodeSelect
        id="report-scope"
        v-model:ids="storeIds"
        multiple
        :nodes="nodes"
        :empty-label="t('report.filter.allWallets')"
        class="w-full @3xl:w-56"
      />
    </div>
    <!-- One visible switch for the sections where it means something. -->
    <div v-if="props.realTerms" class="flex items-center gap-2 self-end pb-2" data-tour="real-terms" :title="t('analytics.realTermsHint')">
      <Switch id="report-real-terms" v-model="filter.real_terms" />
      <Label for="report-real-terms">{{ t('analytics.realTerms') }}</Label>
      <RouterLink
        v-if="filter.real_terms"
        :to="{ query: { ...route.query, settings: 'inflation' } }"
        class="text-xs text-muted-foreground underline"
      >
        {{ t('analytics.realTermsRate') }}
      </RouterLink>
    </div>
    <!-- Rarely changed: folded under «Ещё фильтры». -->
    <template v-if="props.wallets" #more>
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
    </template>
  </FilterBar>
</template>
