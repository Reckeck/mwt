<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { SectionState } from '@/shared/ui/section-state'

import CashflowView from './views/CashflowView.vue'
import CategoriesView from './views/CategoriesView.vue'
import InflationView from './views/InflationView.vue'
import LossesView from './views/LossesView.vue'
import NetworthView from './views/NetworthView.vue'
import PnlView from './views/PnlView.vue'
import PortfolioView from './views/PortfolioView.vue'
import StoresView from './views/StoresView.vue'
import YearView from './views/YearView.vue'
import type { AnalyticsReportViewProps } from './analytics-reports.types'
import { reportIsEmpty } from './report-empty'
import { useReport } from './use-report'

const props = defineProps<AnalyticsReportViewProps>()

const { t } = useI18n()

const report = useReport(
  () => props.kind,
  () => props.filter,
)
const data = computed(() => report.data.value ?? null)
const empty = computed(() => data.value !== null && reportIsEmpty(data.value))
</script>

<template>
  <SectionState
    :loading="report.isPending.value"
    :error="report.error.value"
    :empty="empty"
    :empty-title="t('ui.noData')"
    :empty-description="t('ui.noDataHint')"
    skeleton="chart"
    @retry="report.refetch()"
  >
    <template v-if="data">
      <CashflowView v-if="'cashflow' in data" :report="data.cashflow" />
      <YearView v-else-if="'year' in data" :report="data.year" />
      <CategoriesView v-else-if="'categories' in data" :report="data.categories" />
      <NetworthView v-else-if="'networth' in data" :report="data.networth" />
      <StoresView v-else-if="'stores' in data" :report="data.stores" />
      <PnlView v-else-if="'pnl' in data" :report="data.pnl" />
      <PortfolioView v-else-if="'portfolio' in data" :report="data.portfolio" />
      <InflationView v-else-if="'inflation' in data" :report="data.inflation" />
      <LossesView v-else-if="'losses' in data" :report="data.losses" />
    </template>
  </SectionState>
</template>
