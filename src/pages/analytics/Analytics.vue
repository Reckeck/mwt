<script setup lang="ts">
import { computed } from 'vue'
import { type LocationQuery, RouterView, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { ReportExportButton } from '@/features/report-export'
import { ReportFilterBar, type ReportPeriod } from '@/widgets/report-filter'
import { RouteTabs } from '@/shared/ui/route-tabs'

import {
  type AnalyticsGroup,
  FLOW_VIEW_KIND,
  GROUP_EXPORTS,
  GROUP_ROUTE,
  MONTH_GROUPS,
  flowViewOf,
} from './model/analytics-groups'
import { useReportFilter } from './model/use-report-filter'

const { t } = useI18n()
const route = useRoute()

const group = computed(() => (route.meta.group as AnalyticsGroup | undefined) ?? 'overview')
const view = computed(() => flowViewOf(route.query.view))

const TAB_LABEL: Record<AnalyticsGroup, string> = {
  overview: 'analytics.group.overview',
  flow: 'analytics.group.flow',
  wealth: 'analytics.groupShort.wealth',
  investments: 'analytics.group.investments',
  saldo: 'analytics.group.saldo',
}
const tabs = computed(() =>
  (Object.keys(GROUP_ROUTE) as AnalyticsGroup[]).map((entry) => ({ name: GROUP_ROUTE[entry], label: t(TAB_LABEL[entry]) })),
)
/** The filter rides along in the query, so the next section shows the same period. */
function keepFilter(query: LocationQuery): LocationQuery {
  const { view: _view, ...rest } = query
  return rest
}

const exportKinds = computed(() => (group.value === 'flow' ? [FLOW_VIEW_KIND[view.value]] : GROUP_EXPORTS[group.value]))
const period = computed<ReportPeriod>(() => {
  if (MONTH_GROUPS.includes(group.value)) return 'month'
  return group.value === 'flow' && view.value === 'year' ? 'year' : 'range'
})

const { filter, effective, month, realTerms, narrowed, reset } = useReportFilter(group)
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="w-fit max-w-full" data-tour="section-tabs"><RouteTabs :tabs="tabs" :keep-query="keepFilter" /></div>
      <div v-if="exportKinds.length" data-tour="report-export">
        <ReportExportButton :kinds="exportKinds" :filter="effective" />
      </div>
    </div>
    <div data-tour="report-filter">
      <ReportFilterBar
        v-model="filter"
        v-model:month="month"
        :period="period"
        :real-terms="realTerms"
        :wallets="group !== 'saldo'"
        :active="narrowed"
        @reset="reset"
      />
    </div>
    <div class="min-w-0" data-tour="section-content">
      <RouterView v-slot="{ Component }">
        <component :is="Component" :filter="effective" />
      </RouterView>
    </div>
  </div>
</template>
