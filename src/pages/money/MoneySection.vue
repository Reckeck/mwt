<script setup lang="ts">
import { computed } from 'vue'
import { type LocationQuery, RouterView, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { RouteTabs } from '@/shared/ui/route-tabs'

import {
  ALL_WALLETS_GROUPS,
  FLOW_VIEW_KIND,
  GROUP_EXPORTS,
  GROUP_ROUTE,
  MAIN_GROUPS,
  MORE_GROUPS,
  type MoneyGroup,
  UNFILTERED_GROUPS,
  flowViewOf,
} from './model/money-groups'
import { useReportFilter } from './model/use-report-filter'
import { MoneyToolbar } from './ui/money-toolbar'

const { t } = useI18n()
const route = useRoute()

const group = computed(() => (route.meta.group as MoneyGroup | undefined) ?? 'spending')
const view = computed(() => flowViewOf(route.query.view))

const tab = (entry: MoneyGroup, hint: boolean) => ({
  name: GROUP_ROUTE[entry],
  label: t(`money.tab.${entry}`),
  hint: hint ? t(`money.hint.${entry}`) : undefined,
})
const tabs = computed(() => MAIN_GROUPS.map((entry) => tab(entry, false)))
const more = computed(() => MORE_GROUPS.map((entry) => tab(entry, true)))

/** The filter rides along in the query, so the next tab shows the same period. */
function keepFilter(query: LocationQuery): LocationQuery {
  const { view: _view, ...rest } = query
  return rest
}

const exportKinds = computed(() => (group.value === 'where' ? [FLOW_VIEW_KIND[view.value]] : (GROUP_EXPORTS[group.value] ?? [])))
const { filter, range, fallback, effective, realTerms } = useReportFilter(group)
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="w-fit max-w-full" data-tour="section-tabs">
        <RouteTabs :tabs="tabs" :more="more" :more-label="t('nav.more')" :keep-query="keepFilter" />
      </div>
      <MoneyToolbar
        v-if="!UNFILTERED_GROUPS.includes(group)"
        v-model="filter"
        v-model:range="range"
        :fallback="fallback"
        :real-terms="realTerms"
        :wallets="!ALL_WALLETS_GROUPS.includes(group)"
        :export-kinds="exportKinds"
        :effective="effective"
      />
    </div>
    <div class="min-w-0" data-tour="section-content">
      <RouterView v-slot="{ Component }">
        <component :is="Component" :filter="effective" />
      </RouterView>
    </div>
  </div>
</template>
