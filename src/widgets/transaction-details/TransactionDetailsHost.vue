<script setup lang="ts">
import { txDetailsOpen, txDetailsTx } from '@/shared/lib/tx-details'

import TransactionDetailsSheet from './TransactionDetailsSheet.vue'
import { useLatestTx } from './model/use-latest-tx'

/** The one transaction card of the app, opened from anywhere with `openTxDetails`. */
const latest = useLatestTx(txDetailsTx)
</script>

<template>
  <TransactionDetailsSheet v-model:open="txDetailsOpen" :tx="latest" />
</template>
