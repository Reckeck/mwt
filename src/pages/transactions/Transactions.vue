<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

import { TransactionFilterBar } from '@/features/transaction-filter'
import { TxRowMenuHost, useTxRowMenu } from '@/features/transaction-row-menu'
import { TransactionsList, useTxPages } from '@/entities/transaction'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'
import { openTxDetails } from '@/shared/lib/tx-details'

import { useTxFilterInUrl } from './tx-filter.state'

const { t } = useI18n()
const queryClient = useQueryClient()

const filter = useTxFilterInUrl()

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
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div data-tour="tx-filter">
      <TransactionFilterBar v-model="filter" />
    </div>

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
            :row-actions="rowMenu.actions"
            @open="openDetails"
            @more="txs.fetchNextPage()"
          />
        </div>
      </SectionState>
    </Card>

    <TxRowMenuHost :menu="rowMenu" />
  </div>
</template>
