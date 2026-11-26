<script setup lang="ts">
import { TransactionsExportButton } from '@/features/report-export'
import { SpendingCalendar } from '@/widgets/spending-calendar'

import { sharedTxFilter } from './tx-filter.state'
</script>

<template>
  <div class="grid gap-3">
    <div class="flex justify-end"><TransactionsExportButton :filter="sharedTxFilter" /></div>
    <SpendingCalendar :filter="sharedTxFilter" />
  </div>
</template>
