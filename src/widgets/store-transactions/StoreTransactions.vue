<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { TransactionFilterBar } from '@/features/transaction-filter'
import { TxRowMenuHost, useTxRowMenu } from '@/features/transaction-row-menu'
import { TransactionsList, useTxPages } from '@/entities/transaction'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { storeTxFilter } from '@/shared/lib/tx-filter'

import { openTxDetails } from '@/shared/lib/tx-details'

import type { StoreTransactionsProps } from './store-transactions.types'

const props = withDefaults(defineProps<StoreTransactionsProps>(), { includeChildren: true })

const { t } = useI18n()

/** The wallet is fixed by the page; the rest of the filter is the user's. */
const filter = ref<TxFilter>(storeTxFilter(props.storeId, props.includeChildren))
watch(
  () => [props.storeId, props.includeChildren] as const,
  ([storeId, includeChildren]) => {
    filter.value = { ...filter.value, scope: { store_ids: [storeId], include_children: includeChildren } }
  },
)

/** The day totals are in the base currency: the summary has its rate. */
const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary') })
const { query: txs, rows, total } = useTxPages(filter)

const openDetails = openTxDetails

/** «⋯» and right-click on a row; an edit opens from this wallet's side. */
const rowMenu = useTxRowMenu({ storeId: () => props.storeId, duplicate: true })
</script>

<template>
  <Card>
    <template #header>
      <div class="grid gap-3">
        <p class="font-medium">{{ t('hist.title') }}</p>
        <TransactionFilterBar v-model="filter" fixed-scope id-prefix="store-hist" />
      </div>
    </template>
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
          :store-id="props.storeId"
          openable
          group-by-day
          :fx-per-usd="summary.data.value?.fx_per_usd"
          :base-currency="summary.data.value?.base_currency"
          @open="openDetails"
          :row-actions="rowMenu.actions"
          @more="txs.fetchNextPage()"
        />
      </div>
    </SectionState>
    <TxRowMenuHost :menu="rowMenu" />
  </Card>
</template>
