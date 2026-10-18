<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AnalyticsReportView } from '@/widgets/analytics-reports'
import { FlowList } from '@/widgets/flow-list'

const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
</script>

<template>
  <div class="grid grid-cols-1 gap-6">
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('analytics.section.allocation') }}</h2>
      <AnalyticsReportView kind="portfolio" :filter="props.filter" />
    </section>
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('analytics.section.result') }}</h2>
      <AnalyticsReportView kind="pnl" :filter="props.filter" />
    </section>
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('analytics.section.losses') }}</h2>
      <AnalyticsReportView kind="losses" :filter="props.filter" />
      <!-- How each chain of conversions lost money, under the totals. -->
      <FlowList />
    </section>
  </div>
</template>
