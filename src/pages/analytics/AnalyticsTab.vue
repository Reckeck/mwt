<script setup lang="ts">
import { AnalyticsReportView } from '@/widgets/analytics-reports'
import { FlowList } from '@/widgets/flow-list'

const props = defineProps<{ kind: ReportKind; filter: ReportFilter }>()
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <AnalyticsReportView :kind="props.kind" :filter="props.filter" />
    <!-- Losses: how each chain of conversions lost money, under the totals. -->
    <FlowList v-if="props.kind === 'losses'" />
  </div>
</template>
