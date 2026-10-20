<script setup lang="ts">
import { computed } from 'vue'

import { SaldoReport } from '@/widgets/analytics-reports'
import { localDate } from '@/shared/lib/planning'

const props = defineProps<{ filter: ReportFilter }>()

/** The section's month (`to`, as on the overview); this month by default. */
const month = computed(() => props.filter.to ?? localDate(new Date()).slice(0, 7))
</script>

<template>
  <SaldoReport :month="month" />
</template>
