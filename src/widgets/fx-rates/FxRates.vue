<script setup lang="ts">
import { computed, h, ref, } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { FavoriteStarButton } from '@/features/currency-favorite'
import { Card } from '@/shared/ui/card'
import { type ColumnDef, DataTable } from '@/shared/ui/data-table'
import { Input } from '@/shared/ui/input'
import { call } from '@/shared/api/ipc'
import { formatDateTime, formatQuantity } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

const { t } = useI18n()

const table = useQuery({ queryKey: ['fx-table'], queryFn: () => call<FxTable>('get_fx_table') })
const search = ref('')
const rows = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return [...(table.data.value?.rows ?? [])]
    .filter((row) => !needle || row.currency.includes(needle))
    .sort(
      (left, right) =>
        Number(right.favorite) - Number(left.favorite) ||
        Number(right.tracked) - Number(left.tracked) ||
        left.currency.localeCompare(right.currency),
    )
})

/** A coin is worth thousands of base units, so its «per base» needs more digits. */
const fmt = (value: string, crypto = false) => formatQuantity(value, crypto ? 10 : 6)

/** Favourites, then tracked currencies, then by code; a header click sorts by that column. */
const columns = computed<ColumnDef<FxRow>[]>(() => {
  const base = (table.data.value?.base ?? '').toUpperCase()
  return [
    {
      id: 'favorite',
      header: '',
      enableSorting: false,
      cell: ({ row }) => h(FavoriteStarButton, { code: row.original.currency }),
    },
    {
      id: 'currency',
      header: t('rates.currency'),
      accessorKey: 'currency',
      cell: ({ row }) =>
        h('span', { class: 'inline-flex items-baseline gap-1.5' }, [
          h(
            'span',
            { class: cn('uppercase', row.original.tracked ? 'font-medium' : 'text-muted-foreground') },
            row.original.currency,
          ),
          row.original.crypto
            ? h('span', { class: 'text-2xs uppercase text-muted-foreground' }, t('currency.kind.crypto'))
            : null,
        ]),
    },
    {
      id: 'inBase',
      header: t('rates.inBase', { base }),
      accessorFn: (row) => Number(row.in_base),
      cell: ({ row }) => fmt(row.original.in_base),
      meta: { align: 'right' },
    },
    {
      id: 'perBase',
      header: t('rates.perBase', { base }),
      accessorFn: (row) => Number(row.per_base),
      cell: ({ row }) => fmt(row.original.per_base, row.original.crypto),
      meta: { align: 'right' },
    },
    {
      id: 'updated',
      header: t('rates.updated'),
      accessorFn: (row) => row.updated_at ?? '',
      cell: ({ row }) => (row.original.updated_at ? formatDateTime(row.original.updated_at) : '—'),
      meta: { align: 'right', class: 'text-xs text-muted-foreground' },
    },
  ]
})
</script>

<template>
  <div class="grid grid-cols-1 items-start gap-4">
    <Card>
      <template #header>
        <div class="flex flex-wrap items-center justify-between gap-2">
          <h3 class="heading-card">{{ t('rates.title', { base: (table.data.value?.base ?? '').toUpperCase() }) }}</h3>
          <Input v-model="search" class="h-8 w-40" :placeholder="t('rates.search')" />
        </div>
      </template>
      <!-- The table's own wrapper scrolls: an outer one would leave the sticky header nothing to stick to. -->
      <div>
        <DataTable sticky-header container-class="max-h-[min(32rem,60dvh)]" :columns="columns" :data="rows" :get-row-id="(row) => row.currency" :empty-text="t('rates.empty')" />
      </div>
      <p class="mt-3 text-xs text-muted-foreground">{{ t('rates.hint') }}</p>
    </Card>
  </div>
</template>
