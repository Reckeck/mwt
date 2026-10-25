<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute } from 'vue-router'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

import { CircleCheck } from 'lucide-vue-next'

import { TransactionsExportButton } from '@/features/report-export'
import { TransactionBulkBar } from '@/features/transaction-bulk'
import { SaveFilterButton, TransactionFilterBar } from '@/features/transaction-filter'
import { TxRowMenuHost, useTxRowMenu } from '@/features/transaction-row-menu'
import { TransactionsList, useTxPages } from '@/entities/transaction'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'
import { openTxDetails } from '@/shared/lib/tx-details'
import { backFromQuery } from '@/shared/lib/drill'
import { useIsPhone } from '@/shared/lib/phone'
import { activeFilterCount } from '@/shared/lib/tx-filter'
import { useSelection } from '@/shared/lib/selection'
import { txId } from '@/shared/lib/transactions'
import { BackChip } from '@/shared/ui/back-chip'
import type { UiAction } from '@/shared/ui/context-menu'

import { useTxFilterInUrl } from './tx-filter.state'
import { FilterCount } from './ui/filter-count'
import { FilterTotal } from './ui/filter-total'

const { t } = useI18n()
const queryClient = useQueryClient()
const phone = useIsPhone()

const filter = useTxFilterInUrl()
const route = useRoute()
/** Came from a figure elsewhere («Траты · октябрь»): the chip leads back there. */
const back = computed(() => backFromQuery(route.query))

const { query: txs, rows, total } = useTxPages(filter)

/** The day totals are in the base currency: the summary has its rate. */
const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary') })
const losses = useQuery({
  queryKey: ['transactions', 'losses'],
  queryFn: () => call<Record<string, TxLoss>>('get_tx_losses'),
})

const openDetails = openTxDetails

/**
 * New rates change the day totals (their rate comes with the summary). The rows
 * themselves change only when operations got prices stamped (`restamped`), and
 * the shell already refreshes the list then: a rate fetched while the user types
 * in the transaction form must not reload the whole list under the dialog.
 */
useBackendEvent<RefreshResult>('prices-updated', () => {
  void queryClient.invalidateQueries({ queryKey: ['summary'] })
})

/* --- «⋯» and right-click on a row: open, edit, duplicate, copy, delete --- */
const rowMenu = useTxRowMenu({ duplicate: true })

/* --- Several rows at once (Ops-1 · 4): checkboxes on a desktop, «Выбрать» in the hold menu on a phone --- */
const selection = useSelection(() => rows.value.map(txId))
function rowActions(tx: Transaction): UiAction[] {
  if (!phone.value) return rowMenu.actions(tx)
  const pick: UiAction = { id: 'pick', label: t('bulk.pickOne'), icon: CircleCheck, run: () => selection.toggle(txId(tx)) }
  return [pick, ...rowMenu.actions(tx)]
}
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div v-if="back || activeFilterCount(filter)" class="flex flex-wrap items-center gap-x-3 gap-y-2">
      <BackChip v-if="back" :to="back.to" :label="back.label" />
      <FilterTotal
        v-if="!txs.isPending.value"
        class="ms-auto"
        :rows="rows"
        :total="total"
        :kinds="filter.kinds"
        :fx-per-usd="summary.data.value?.fx_per_usd"
        :base-currency="summary.data.value?.base_currency"
      />
    </div>
    <div data-tour="tx-filter">
      <TransactionFilterBar v-model="filter">
        <template v-if="!phone" #actions>
          <TransactionsExportButton :filter="filter" />
          <!-- Only a filter that narrows something is worth keeping. -->
          <SaveFilterButton v-if="activeFilterCount(filter)" :filter="filter" />
        </template>
        <template #trail>
          <FilterCount v-if="!txs.isPending.value" :filter="filter" :total="total" />
        </template>
      </TransactionFilterBar>
    </div>

    <TransactionBulkBar v-if="selection.active.value" :selection="selection" :transactions="rows" />

    <Card>
      <SectionState
        :loading="txs.isPending.value"
        :error="txs.error.value"
        :empty="!rows.length"
        :empty-title="t('filter.empty')"
        :empty-description="t('ui.noDataHint')"
        compact
        @retry="txs.refetch()"
      >
        <div class="-my-3">
          <TransactionsList
            :transactions="rows"
            :total="total"
            :loading-more="txs.isFetchingNextPage.value"
            :losses="losses.data.value ?? null"
            openable
            group-by-day
            :fx-per-usd="summary.data.value?.fx_per_usd"
            :base-currency="summary.data.value?.base_currency"
            :row-actions="rowActions"
            :selection="selection"
            @open="openDetails"
            @more="txs.fetchNextPage()"
          >
          </TransactionsList>
        </div>
      </SectionState>
    </Card>

    <TxRowMenuHost :menu="rowMenu" />
  </div>
</template>
