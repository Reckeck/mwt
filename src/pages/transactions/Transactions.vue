<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute } from 'vue-router'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { Search } from 'lucide-vue-next'

import { TransactionFilter, useFilterChips } from '@/features/transaction-filter'
import { TransactionsList, useTxPages } from '@/entities/transaction'
import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'
import { backFromQuery } from '@/shared/lib/drill'
import { activeFilterCount, emptyTxFilter } from '@/shared/lib/tx-filter'
import { openTxDetails } from '@/shared/lib/tx-details'
import { BackChip } from '@/shared/ui/back-chip'
import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'

import { useTxFilterInUrl } from './tx-filter.state'
import { ResultTotal } from './ui/result-total'

const { t } = useI18n()
const queryClient = useQueryClient()
const route = useRoute()

const filter = useTxFilterInUrl()
/** Came from a figure elsewhere (`Spending · October`): the chip leads back there. */
const back = computed(() => backFromQuery(route.query))

const { query: txs, rows, total } = useTxPages(filter)

/** The day totals are in the base currency: the summary has its rate. */
const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary'), placeholderData: (previous) => previous })
const losses = useQuery({
  queryKey: ['transactions', 'losses'],
  queryFn: () => call<Record<string, TxLoss>>('get_tx_losses'),
  placeholderData: (previous) => previous,
})

/** Nothing kept: what narrowed it (the search, the filters by name) and one button that takes both off. */
const chipsOf = useFilterChips()
const narrowed = computed(() => {
  const text = filter.value.text?.trim() ?? ''
  const filters = chipsOf(filter.value)
    .map((chip) => chip.label)
    .join(', ')
  if (text && filters) return { hint: t('filter.emptyBoth', { text, filters }), reset: t('filter.resetBoth') }
  if (text) return { hint: t('filter.emptySearch', { text }), reset: t('filter.resetSearch') }
  if (filters) return { hint: t('filter.emptyFilter', { filters }), reset: t('filter.resetFilter') }
  return null
})

/** New rates change the day totals (their rate comes with the summary); the rows stay. */
useBackendEvent<RefreshResult>('prices-updated', () => {
  void queryClient.invalidateQueries({ queryKey: ['summary'] })
})
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)] gap-4 @container">
    <div v-if="back" class="flex flex-wrap items-center gap-x-3 gap-y-2">
      <BackChip :to="back.to" :label="back.label" />
    </div>
    <div data-tour="tx-filter">
      <TransactionFilter v-model="filter">
        <template #trail>
          <ResultTotal
            v-if="!txs.isPending.value"
            :rows="rows"
            :total="total"
            :narrowed="activeFilterCount(filter) > 0"
            :fx-per-usd="summary.data.value?.fx_per_usd"
            :base-currency="summary.data.value?.base_currency"
          />
        </template>
      </TransactionFilter>
    </div>

    <SectionState
      skeleton="operations"
      :loading="txs.isPending.value"
      :error="txs.error.value"
      :empty="!rows.length"
      :empty-title="narrowed ? t('filter.empty') : t('tx.empty')"
      :empty-description="narrowed?.hint"
      :empty-icon="narrowed ? Search : undefined"
      compact
      @retry="txs.refetch()"
    >
      <template v-if="narrowed" #empty>
        <Button variant="outline" class="touch-min" data-filter-empty-reset @click="filter = emptyTxFilter()">{{ narrowed.reset }}</Button>
      </template>
      <TransactionsList
        :transactions="rows"
        :total="total"
        :loading-more="txs.isFetchingNextPage.value"
        :losses="losses.data.value ?? null"
        :fx-per-usd="summary.data.value?.fx_per_usd"
        :base-currency="summary.data.value?.base_currency"
        @open="openTxDetails"
        @more="txs.fetchNextPage()"
      />
    </SectionState>
  </div>
</template>
