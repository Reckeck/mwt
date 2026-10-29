<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { TxRowMenuHost, useTxRowMenu } from '@/features/transaction-row-menu'
import { TransactionsList, useTxPages } from '@/entities/transaction'
import { call } from '@/shared/api/ipc'
import { hereBackQuery } from '@/shared/lib/drill'
import { formatDayMonth } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { txMeta } from '@/shared/lib/transactions'
import { openTxDetails } from '@/shared/lib/tx-details'
import { emptyTxFilter, txFilterToQuery } from '@/shared/lib/tx-filter'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

/**
 * `What changed` on Home (Home 1): the operations since the last visit, by day
 * with the day's net, each opening its card. The links lead to the full journal
 * of the same days and to the spending.
 */
const props = defineProps<{ since: string }>()
const { t } = useI18n()

const filter = ref<TxFilter>(emptyTxFilter({ from: props.since }))
const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary') })
const { query: txs, rows } = useTxPages(filter)
const phone = useIsPhone()
/**
 * The desktop card has a fixed height and scrolls inside it, so every day since the visit is there;
 * the phone scrolls the page, so it shows the latest few days and the rest is behind `All operations`.
 */
const DAYS_SHOWN = 3
const visible = computed(() => {
  if (!phone.value) return rows.value
  const days = new Set<string>()
  const out: Transaction[] = []
  for (const tx of rows.value) {
    days.add(txMeta(tx).timestamp.slice(0, 10))
    if (days.size > DAYS_SHOWN) break
    out.push(tx)
  }
  return out
})
const rowMenu = useTxRowMenu({ duplicate: true })
const sinceText = computed(() => formatDayMonth(`${props.since}T12:00`))
</script>

<template>
  <Card data-tour="home-changes">
    <template #header>
      <div class="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-1">
        <h2 class="text-base font-semibold">{{ t('home.changes.title') }}</h2>
        <span class="text-sm text-muted-foreground max-md:hidden">{{ t('home.changes.since', { date: sinceText }) }}</span>
        <span class="flex-1" />
        <RouterLink :to="{ name: 'transactions', query: { ...txFilterToQuery(filter), ...hereBackQuery() } }" class="text-sm text-primary hover:underline">
          <span class="max-md:hidden">{{ t('home.changes.all') }} →</span>
          <span class="md:hidden">{{ t('home.changes.allShort') }}</span>
        </RouterLink>
        <RouterLink :to="{ name: 'money' }" class="text-sm text-primary hover:underline max-md:hidden">
          {{ t('home.changes.spending') }} →
        </RouterLink>
      </div>
    </template>
    <SectionState
      class="pt-1"
      :loading="txs.isPending.value"
      :error="txs.error.value"
      :empty="!rows.length"
      :empty-title="t('home.changes.nothing', { date: sinceText })"
      :empty-description="t('home.changes.nothingHint')"
      compact
      @retry="txs.refetch()"
    >
      <!-- The header keeps its 8px; pt-1 of the state makes the band start 12px under the title, and the last row sits on the card's padding. -->
      <div class="-mb-3 md:-mx-4 md:max-h-96 md:overflow-y-auto md:overscroll-contain md:px-4" data-scroll-list>
        <TransactionsList
          :transactions="visible"
          :total="visible.length"
          :loading-more="txs.isFetchingNextPage.value"
          openable
          group-by-day
          :fx-per-usd="summary.data.value?.fx_per_usd"
          :base-currency="summary.data.value?.base_currency"
          :row-actions="rowMenu.actions"
          @open="openTxDetails"
          @more="txs.fetchNextPage()"
        >
        </TransactionsList>
      </div>
    </SectionState>
    <TxRowMenuHost :menu="rowMenu" />
  </Card>
</template>
