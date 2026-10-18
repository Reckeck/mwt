<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { ReportExportButton } from '@/features/report-export'
import { ReportFilterBar, type ReportPeriod } from '@/widgets/report-filter'
import { Tabs } from '@/shared/ui/tabs'

import { type AnalyticsGroup, FLOW_VIEW_KIND, GROUP_EXPORTS, GROUP_ROUTE, flowViewOf } from './model/analytics-groups'
import { useReportFilter } from './model/use-report-filter'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const group = computed(() => (route.meta.group as AnalyticsGroup | undefined) ?? 'overview')
const view = computed(() => flowViewOf(route.query.view))

const TAB_LABEL: Record<AnalyticsGroup, string> = {
  overview: 'analytics.group.overview',
  flow: 'analytics.group.flow',
  wealth: 'analytics.groupShort.wealth',
  investments: 'analytics.group.investments',
}
const tabs = computed(() =>
  (Object.keys(GROUP_ROUTE) as AnalyticsGroup[]).map((entry) => ({ value: GROUP_ROUTE[entry], label: t(TAB_LABEL[entry]) })),
)
const tab = computed({
  get: () => GROUP_ROUTE[group.value],
  // The filter rides along in the query, so the next section shows the same period.
  set: (name: string) => {
    const { view: _view, ...query } = route.query
    void router.push({ name, query })
  },
})

const exportKinds = computed(() => (group.value === 'flow' ? [FLOW_VIEW_KIND[view.value]] : GROUP_EXPORTS[group.value]))
const period = computed<ReportPeriod>(() => {
  if (group.value === 'overview') return 'month'
  return group.value === 'flow' && view.value === 'year' ? 'year' : 'range'
})

const { filter, narrowed, reset } = useReportFilter()
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="w-fit max-w-full" data-tour="section-tabs"><Tabs v-model="tab" :tabs="tabs" /></div>
      <div v-if="exportKinds.length" data-tour="report-export">
        <ReportExportButton :kinds="exportKinds" :filter="filter" />
      </div>
    </div>
    <div data-tour="report-filter">
      <ReportFilterBar
        v-model="filter"
        :period="period"
        :scoped="!(group === 'flow' && view === 'year')"
        :real-terms="group === 'flow'"
        :active="narrowed"
        @reset="reset"
      />
    </div>
    <div class="min-w-0" data-tour="section-content">
      <RouterView v-slot="{ Component }">
        <component :is="Component" :filter="filter" />
      </RouterView>
    </div>
  </div>
</template>
