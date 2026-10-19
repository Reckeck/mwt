<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { quickAddOpen } from '@/shared/lib/quick-add'

import { ReportInsights } from '@/entities/report'
import { Button } from '@/shared/ui/button'
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
import { EMPTY_ACTION, reportIsEmpty } from './report-empty'
import { useReport } from './use-report'

const props = defineProps<AnalyticsReportViewProps>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

/** A filter that narrows the report may be why it is empty: then «reset» comes first. */
const FILTER_KEYS = ['store', 'from', 'to', 'year', 'cur']
const narrowed = computed(() => FILTER_KEYS.some((key) => route.query[key] !== undefined))
function resetFilter() {
  const query = { ...route.query }
  for (const key of FILTER_KEYS) delete query[key]
  void router.replace({ query })
}
const action = computed(() => EMPTY_ACTION[props.kind])
function runAction() {
  if (action.value === 'createWallet') void router.push({ name: 'stores' })
  else if (action.value === 'inflation') void router.replace({ query: { ...route.query, settings: 'inflation' } })
  else quickAddOpen.value = true
}

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
    <template #empty>
      <div class="flex flex-wrap justify-center gap-2">
        <Button v-if="narrowed" variant="outline" @click="resetFilter">{{ t('report.emptyAction.resetFilter') }}</Button>
        <Button @click="runAction">{{ t(`report.emptyAction.${action}`) }}</Button>
      </div>
    </template>
    <div v-if="data" class="grid grid-cols-1 gap-4">
      <Card v-if="meta?.insights.length && !props.hideInsights" data-tour="report-insights">
        <ReportInsights :insights="meta.insights" :currency="meta.currency" />
      </Card>
      <CashflowView v-if="'cashflow' in data" :report="data.cashflow" :filter="props.filter" />
      <YearView v-else-if="'year' in data" :report="data.year" :filter="props.filter" />
      <CategoriesView v-else-if="'categories' in data" :report="data.categories" :filter="props.filter" />
      <NetworthView v-else-if="'networth' in data" :report="data.networth" />
      <StoresView v-else-if="'stores' in data" :report="data.stores" />
      <PnlView v-else-if="'pnl' in data" :report="data.pnl" :filter="props.filter" />
      <PortfolioView v-else-if="'portfolio' in data" :report="data.portfolio" />
      <InflationView v-else-if="'inflation' in data" :report="data.inflation" />
      <LossesView v-else-if="'losses' in data" :report="data.losses" :filter="props.filter" />
      <!-- One note for every report: the figures are all in this currency. -->
      <p v-if="meta" class="text-xs text-muted-foreground">
        {{ t('report.currencyNote', { currency: meta.currency.toUpperCase() }) }}
      </p>
    </div>
  </SectionState>
</template>
