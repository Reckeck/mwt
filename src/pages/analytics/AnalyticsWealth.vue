<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { AnalyticsReportView } from '@/widgets/analytics-reports'

const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <AnalyticsReportView kind="networth" :filter="props.filter" />
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('report.byWallet') }}</h2>
      <AnalyticsReportView kind="stores" :filter="props.filter" />
    </section>
  </div>
</template>
