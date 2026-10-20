<script setup lang="ts">
import { computed, h } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { RefreshCw } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { type ColumnDef, DataTable } from '@/shared/ui/data-table'
import { call, parseError } from '@/shared/api/ipc'
import { formatQuantity } from '@/shared/lib/format'

/**
 * Bybit's own balances per account next to what the ledger booked on the
 * matching sub-account. A difference is history the import has not seen
 * (a deposit, a withdrawal, an older trade) — the user decides what it was.
 */
const props = defineProps<{ storeId: string }>()
const { t } = useI18n()

const balances = useQuery({
  // no placeholder: another exchange's balances
  queryKey: computed(() => ['bybit-balances', props.storeId]),
  queryFn: () => call<ExchangeAccountBalance[]>('bybit_balances', { storeId: props.storeId }),
  enabled: false,
  retry: false,
})
/** Matching rows are calm; more on the exchange is history to book, less is a mistake to find. */
const tone = (diff: string) =>
  Number(diff) === 0 ? 'text-muted-foreground' : Number(diff) > 0 ? 'text-warning' : 'text-destructive'

const columns = computed<ColumnDef<ExchangeCoinRow>[]>(() => [
  { id: 'coin', header: t('exchangeBalances.coin'), accessorKey: 'coin', meta: { class: 'font-medium' } },
  {
    id: 'exchange',
    header: t('exchangeBalances.exchange'),
    accessorFn: (row) => Number(row.exchange),
    cell: ({ row }) => formatQuantity(row.original.exchange),
    meta: { align: 'right' },
  },
  {
    id: 'ledger',
    header: t('exchangeBalances.ledger'),
    accessorFn: (row) => Number(row.ledger),
    cell: ({ row }) => formatQuantity(row.original.ledger),
    meta: { align: 'right' },
  },
  {
    id: 'diff',
    header: t('exchangeBalances.diff'),
    accessorFn: (row) => Number(row.diff),
    cell: ({ row }) =>
      h(
        'span',
        { class: tone(row.original.diff) },
        Number(row.original.diff) === 0 ? '✓' : formatQuantity(row.original.diff),
      ),
    meta: { align: 'right' },
  },
])
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <h3 data-widget-title class="heading-card">{{ t('exchangeBalances.title') }}</h3>
        <Button size="sm" variant="outline" :disabled="balances.isFetching.value" @click="balances.refetch()">
          <RefreshCw class="size-4" :class="balances.isFetching.value && 'animate-spin'" />
          {{ t('exchangeBalances.load') }}
        </Button>
      </div>
    </template>
    <p v-if="balances.isError.value" class="text-sm text-destructive">
      {{ t('exchangeBalances.failed') }}: {{ parseError(balances.error.value).message }}
    </p>
    <div v-else-if="balances.data.value" class="grid gap-4 @wide:grid-cols-3">
      <section v-for="account in balances.data.value" :key="account.account" class="grid content-start gap-1.5">
        <p class="text-sm font-medium">{{ t(`exchangeBalances.account.${account.account}`) }}</p>
        <p v-if="!account.store_id" class="text-xs text-muted-foreground">{{ t('exchangeBalances.noAccount') }}</p>
        <p v-if="!account.rows.length" class="text-xs text-muted-foreground">{{ t('exchangeBalances.empty') }}</p>
        <DataTable v-else class="text-xs" :columns="columns" :data="account.rows" :get-row-id="(row) => row.coin" />
      </section>
    </div>
  </Card>
</template>
