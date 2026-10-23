<script setup lang="ts">
import { SpendingCalendar } from '@/widgets/spending-calendar'

import { sharedTxFilter } from './tx-filter.state'
</script>

<template>
  <SpendingCalendar :filter="sharedTxFilter" header-to="#tx-section-end" />
</template>
