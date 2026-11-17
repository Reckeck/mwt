<script setup lang="ts">
import { computed } from 'vue'

import { useCashbackReport, useStoreTree } from '@/shared/api/queries'
import { SectionState } from '@/shared/ui/section-state'
import { BonusPrograms } from '@/widgets/bonus-programs'
import { CashbackBonuses } from '@/widgets/cashback-bonuses'

/**
 * Bonuses and cashback (Ops-2 · 12): the programmes, then what the cards brought back.
 *
 * The page waits for everything its two widgets draw from (the store tree, the year report
 * of the programmes, the month report of the cashback card) and paints once, in its final
 * layout; the widgets then read the same cache entries, so nothing is fetched twice and
 * nothing below a late block moves.
 */
const tree = useStoreTree()
const year = useCashbackReport('year')
const month = useCashbackReport('month')
const queries = [tree, year, month]

const loading = computed(() => queries.some((query) => query.isPending.value))
const error = computed(() => queries.find((query) => query.error.value)?.error.value)
function retry() {
  for (const query of queries) if (query.error.value) void query.refetch()
}
</script>

<template>
  <SectionState :loading="loading" :error="error" skeleton="list" :rows="4" @retry="retry">
    <div class="grid grid-cols-1 gap-6">
      <BonusPrograms add-to="#tx-section-end" />
      <div data-tour="bonuses-cashback"><CashbackBonuses /></div>
    </div>
  </SectionState>
</template>
