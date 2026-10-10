<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { parseError } from '@/shared/api/ipc'
import { provideChartTheme } from '@/shared/lib/echarts'
import { useTheme } from '@/shared/lib/theme'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import CashflowView from './views/CashflowView.vue'
import CategoriesView from './views/CategoriesView.vue'
import InflationView from './views/InflationView.vue'
import NetworthView from './views/NetworthView.vue'
import PnlView from './views/PnlView.vue'
import PortfolioView from './views/PortfolioView.vue'
import StoresView from './views/StoresView.vue'
import YearView from './views/YearView.vue'
import type { AnalyticsReportViewProps } from './analytics-reports.types'
import { useReport } from './use-report'

const props = defineProps<AnalyticsReportViewProps>()

const { t } = useI18n()
const { resolved } = useTheme()
provideChartTheme(resolved.value)

const report = useReport(
  () => props.kind,
  () => props.filter,
)
const data = computed(() => report.data.value ?? null)
</script>

<template>
  <Skeleton v-if="report.isPending.value" class="h-80 w-full" />
  <Card v-else-if="report.isError.value">
    <p class="text-sm text-destructive">{{ codeErrorMessage(parseError(report.error.value), t) }}</p>
  </Card>
  <template v-else-if="data">
    <CashflowView v-if="'cashflow' in data" :report="data.cashflow" />
    <YearView v-else-if="'year' in data" :report="data.year" />
    <CategoriesView v-else-if="'categories' in data" :report="data.categories" />
    <NetworthView v-else-if="'networth' in data" :report="data.networth" />
    <StoresView v-else-if="'stores' in data" :report="data.stores" />
    <PnlView v-else-if="'pnl' in data" :report="data.pnl" />
    <PortfolioView v-else-if="'portfolio' in data" :report="data.portfolio" />
    <InflationView v-else-if="'inflation' in data" :report="data.inflation" />
  </template>
</template>
