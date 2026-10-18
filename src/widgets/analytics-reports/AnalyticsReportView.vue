<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { ReportInsights } from '@/entities/report'
import { Card } from '@/shared/ui/card'
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
/** Every report variant carries the same meta: currency and the facts in words. */
const meta = computed(() => (data.value ? Object.values(data.value)[0].meta : null))
</script>

<template>
  <SectionState
    :loading="report.isPending.value"
    :error="report.error.value"
    :empty="empty"
    :empty-title="t(`report.emptyState.${props.kind}.title`)"
    :empty-description="t(`report.emptyState.${props.kind}.description`)"
    skeleton="chart"
    @retry="report.refetch()"
  >
    <div v-if="data" class="grid grid-cols-1 gap-4">
      <Card v-if="meta?.insights.length && !props.hideInsights" data-tour="report-insights">
        <ReportInsights :insights="meta.insights" :currency="meta.currency" />
      </Card>
      <CashflowView v-if="'cashflow' in data" :report="data.cashflow" :filter="props.filter" />
      <YearView v-else-if="'year' in data" :report="data.year" :filter="props.filter" />
      <CategoriesView v-else-if="'categories' in data" :report="data.categories" :filter="props.filter" />
      <NetworthView v-else-if="'networth' in data" :report="data.networth" />
      <StoresView v-else-if="'stores' in data" :report="data.stores" />
      <PnlView v-else-if="'pnl' in data" :report="data.pnl" />
      <PortfolioView v-else-if="'portfolio' in data" :report="data.portfolio" />
      <InflationView v-else-if="'inflation' in data" :report="data.inflation" />
      <LossesView v-else-if="'losses' in data" :report="data.losses" />
    </div>
  </SectionState>
</template>
