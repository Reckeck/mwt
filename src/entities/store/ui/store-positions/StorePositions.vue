<script setup lang="ts">
import { computed, h, useSlots } from 'vue'
import { useI18n } from 'vue-i18n'

import { PnlText } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { DataTable, type ColumnDef } from '@/shared/ui/data-table'
import { EmptyState } from '@/shared/ui/empty-state'
import { CHAIN_LABELS } from '@/shared/lib/chains'
import { formatMoney, formatPrice, formatQuantity } from '@/shared/lib/format'

import type { StorePositionsProps } from './store-positions.types'

const props = withDefaults(defineProps<StorePositionsProps>(), { title: '' })

/** Per-row actions (revalue a property…) come from the page. */
defineSlots<{ actions?(props: { position: StorePosition }): unknown }>()
const slots = useSlots()

const { t } = useI18n()

const base = computed(() => props.baseCurrency.toLowerCase())
const money = (value: string) => formatMoney(value, { currency: base.value, decimals: 2 })
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
    cell: ({ row }) => (row.original.avg_cost ? formatPrice(row.original.avg_cost, base.value) : '—'),
    meta: { align: 'right', class: 'text-muted-foreground' },
  },
  {
    id: 'price',
    header: t('pos.price'),
    accessorFn: (row) => Number(row.price_base),
    cell: ({ row }) => (Number(row.original.price_base) > 0 ? formatPrice(row.original.price_base, base.value) : '—'),
    meta: { align: 'right', class: 'text-muted-foreground' },
  },
  {
    id: 'value',
    header: t('pos.value'),
    accessorFn: (row) => Number(row.value_base),
    cell: ({ row }) => money(row.original.value_base),
    meta: { align: 'right', class: 'font-medium' },
  },
  {
    id: 'pnl',
    header: t('pos.pnl'),
    accessorFn: (row) => Number(row.unrealized_base),
    cell: ({ row }) =>
      row.original.avg_cost ? h(PnlText, { amount: row.original.unrealized_base, currency: base.value, decimals: 2 }) : '—',
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
</script>

<template>
  <Card>
    <template #header>
      <h3 class="heading-card">{{ props.title || t('pos.title') }}</h3>
    </template>

    <div v-if="props.positions.length" class="-mx-1 overflow-x-auto">
      <DataTable
        class="min-w-[34rem]"
        :columns="columns"
        :data="props.positions"
        :get-row-id="rowId"
        :initial-sorting="[{ id: 'value', desc: true }]"
      />
    </div>
    <EmptyState v-else compact :title="t('pos.empty')" />
  </Card>
</template>
