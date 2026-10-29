<script setup lang="ts">
import { computed, h, useSlots } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money, PnlText } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { DataTable, type ColumnDef } from '@/shared/ui/data-table'
import { EmptyState } from '@/shared/ui/empty-state'
import { CHAIN_LABELS } from '@/shared/lib/chains'
import { formatDateTime, formatPct, formatQuantity } from '@/shared/lib/format'

import { priceOriginText } from '../../lib/price-origin'
import type { StorePositionsProps } from './store-positions.types'

const props = withDefaults(defineProps<StorePositionsProps>(), { title: '', clickable: false, pricesAsOf: null })
const emit = defineEmits<{ open: [position: StorePosition] }>()

/** Per-row actions (revalue a property…) come from the page. */
defineSlots<{ actions?(props: { position: StorePosition }): unknown }>()
const slots = useSlots()

const { t } = useI18n()

const base = computed(() => props.baseCurrency.toLowerCase())
/** A sum in the base currency, with the conversions on hover. */
const money = (value: string, decimals?: number) => h(Money, { amount: value, currency: base.value, decimals })
/** The result against what was paid, in percent; none when the cost is unknown or zero. */
const pnlPct = (position: StorePosition) => {
  const paid = Number(position.avg_cost ?? 0) * Math.abs(Number(position.quantity))
  return paid ? (Number(position.unrealized_base) / paid) * 100 : null
}
const rowId = (position: StorePosition) => `${position.asset.symbol}-${position.asset.contract ?? ''}`
const where = (position: StorePosition) =>
  position.asset.chain && position.asset.chain !== 'fiat' ? CHAIN_LABELS[position.asset.chain] : position.asset.name

/** Biggest holdings first; every column sorts by its number, not by its text. */
const columns = computed<ColumnDef<StorePosition>[]>(() => [
  {
    id: 'asset',
    header: t('pos.asset'),
    accessorFn: (row) => row.asset.symbol.toUpperCase(),
    cell: ({ row }) =>
      h('div', { class: 'flex min-w-0 items-center gap-2' }, [
        h('span', { class: 'font-medium' }, row.original.asset.symbol.toUpperCase()),
        h('span', { class: 'truncate text-xs text-muted-foreground' }, where(row.original)),
      ]),
  },
  {
    id: 'quantity',
    header: t('pos.quantity'),
    accessorFn: (row) => Number(row.quantity),
    cell: ({ row }) => formatQuantity(row.original.quantity, props.decimals),
    meta: { align: 'right' },
  },
  {
    id: 'avgCost',
    header: t('pos.avgCost'),
    accessorFn: (row) => Number(row.avg_cost ?? 0),
    cell: ({ row }) => (row.original.avg_cost ? money(row.original.avg_cost) : '—'),
    meta: { align: 'right', class: 'text-muted-foreground' },
  },
  {
    id: 'price',
    header: t('pos.price'),
    accessorFn: (row) => Number(row.price_base),
    // Where the price came from rides along as its title: «CoinGecko · 12:05», `your own price`.
    cell: ({ row }) =>
      Number(row.original.price_base) > 0
        ? h('span', { title: priceOriginText(row.original.price_info) }, [money(row.original.price_base)])
        : '—',
    meta: { align: 'right', class: 'text-muted-foreground' },
  },
  {
    id: 'change24h',
    header: t('pos.change24h'),
    accessorFn: (row) => Number(row.change_24h_pct ?? 0),
    cell: ({ row }) =>
      row.original.change_24h_pct === null
        ? '—'
        : h(
            'span',
            { class: Number(row.original.change_24h_pct) < 0 ? 'text-destructive' : 'text-success' },
            formatPct(row.original.change_24h_pct, { signed: true }),
          ),
    meta: { align: 'right' },
  },
  {
    id: 'value',
    header: t('pos.value'),
    accessorFn: (row) => Number(row.value_base),
    cell: ({ row }) => money(row.original.value_base, 2),
    meta: { align: 'right', class: 'font-medium' },
  },
  {
    id: 'pnl',
    header: t('pos.pnl'),
    accessorFn: (row) => Number(row.unrealized_base),
    cell: ({ row }) =>
      row.original.avg_cost
        ? h(PnlText, { amount: row.original.unrealized_base, currency: base.value, decimals: 2, pct: pnlPct(row.original) })
        : '—',
    meta: { align: 'right' },
  },
  ...(slots.actions
    ? [
        {
          id: 'actions',
          header: '',
          enableSorting: false,
          cell: ({ row }) => slots.actions?.({ position: row.original }),
          meta: { align: 'right' },
        } satisfies ColumnDef<StorePosition>,
      ]
    : []),
])
// check-ui-ignore fixed-width: the table scrolls sideways inside its own wrapper on a phone
</script>

<template>
  <Card>
    <template #header>
      <div class="flex flex-wrap items-baseline justify-between gap-2">
        <h3 class="heading-card">{{ props.title || t('pos.title') }}</h3>
        <span v-if="props.pricesAsOf" class="text-xs text-muted-foreground">
          {{ t('pos.pricesAsOf', { time: formatDateTime(props.pricesAsOf) }) }}
        </span>
      </div>
    </template>

    <div v-if="props.positions.length" class="-mx-1 overflow-x-auto">
      <DataTable
        class="min-w-[34rem]"
        sticky-first-column
        :columns="columns"
        :data="props.positions"
        :get-row-id="rowId"
        :initial-sorting="[{ id: 'value', desc: true }]"
        :clickable="props.clickable"
        @row-click="(position) => emit('open', position)"
      />
    </div>
    <EmptyState v-else compact :title="t('pos.empty')" />
  </Card>
</template>
