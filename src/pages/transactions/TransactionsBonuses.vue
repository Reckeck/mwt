<script setup lang="ts">
import { computed } from 'vue'

import { useCashbackReport, useStoreTree } from '@/shared/api/queries'
import { SectionState } from '@/shared/ui/section-state'
import { CashbackBonuses } from '@/widgets/cashback-bonuses'

/**
 * Bonuses and cashback: what the cards brought back and the programmes' balances. The page waits
 * for the queries its widget reads and paints once; the widget reads the same cache entries.
 */
const tree = useStoreTree()
const month = useCashbackReport('month')
const queries = [tree, month]

const loading = computed(() => queries.some((query) => query.isPending.value))
const error = computed(() => queries.find((query) => query.error.value)?.error.value)
function retry() {
  for (const query of queries) if (query.error.value) void query.refetch()
}
</script>

<template>
  <SectionState :loading="loading" :error="error" skeleton="list" :rows="4" @retry="retry">
    <div data-tour="bonuses-cashback"><CashbackBonuses /></div>
  </SectionState>
</template>
