<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { AnalyticsReportView } from '@/widgets/analytics-reports'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { FLOW_VIEWS, FLOW_VIEW_KIND, type FlowView, flowViewOf } from './model/money-groups'

const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

/** The view lives in the address: Back and a reload keep it. */
const view = computed({
  get: () => flowViewOf(route.query.view),
  set: (next: FlowView | undefined) => {
    if (next) void router.replace({ query: { ...route.query, view: next === 'months' ? undefined : next } })
  },
})
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <ToggleGroup
      v-model="view"
      type="single"
      variant="outline"
      size="sm"
      class="w-fit"
      :aria-label="t('analytics.view.label')"
      data-tour="report-view"
    >
      <ToggleGroupItem v-for="entry in FLOW_VIEWS" :key="entry" :value="entry" class="px-3">
        {{ t(`analytics.view.${entry}`) }}
      </ToggleGroupItem>
    </ToggleGroup>
    <AnalyticsReportView :kind="FLOW_VIEW_KIND[view]" :filter="props.filter" />
  </div>
</template>
