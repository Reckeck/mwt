<script setup lang="ts">
import { AnalyticsReportView } from '@/widgets/analytics-reports'

const props = defineProps<{ kind: ReportKind; filter: ReportFilter }>()
</script>

<template>
  <AnalyticsReportView :kind="props.kind" :filter="props.filter" />
</template>
