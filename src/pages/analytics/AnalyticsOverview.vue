<script setup lang="ts">
import { AnalyticsOverview } from '@/widgets/analytics-overview'

const props = defineProps<{ filter: ReportFilter }>()
</script>

<template>
  <AnalyticsOverview :filter="props.filter" />
</template>
