<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AnalyticsReportView } from '@/widgets/analytics-reports'
import { FlowList } from '@/widgets/flow-list'
import { TopupAdvisor } from '@/widgets/topup-advisor'
import { defaultRange } from '@/shared/lib/period'

const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()

/** The chains of the same period the reports show: an unset end is the report's default. */
const period = computed(() => {
  const fallback = defaultRange()
  return { from: props.filter.from ?? fallback.from, to: props.filter.to ?? fallback.to }
})
</script>

<template>
  <div class="grid grid-cols-1 gap-6">
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('analytics.section.allocation') }}</h2>
      <AnalyticsReportView kind="portfolio" :filter="props.filter" />
    </section>
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('analytics.section.topup') }}</h2>
      <TopupAdvisor />
    </section>
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('analytics.section.result') }}</h2>
      <AnalyticsReportView kind="pnl" :filter="props.filter" />
    </section>
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('analytics.section.losses') }}</h2>
      <AnalyticsReportView kind="losses" :filter="props.filter" />
      <!-- How each chain of conversions lost money, under the totals. -->
      <FlowList :from="period.from" :to="period.to" :store-ids="props.filter.scope.store_ids" />
    </section>
  </div>
</template>
