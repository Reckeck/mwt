<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { formatPct } from '@/shared/lib/format'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

/**
 * What the wallets hold per currency (Currency-1 · 2). Under the table it is one
 * line — «₽ 3 295 090 · 35 147 $ · 87 %»; in the «По валютам» view, a table.
 */
const props = defineProps<{
  assets: (AssetBalance & { share: number })[]
  base: string
  table?: boolean
}>()
const { t } = useI18n()
const pct = (share: number) => formatPct(share * 100, { decimals: 0 })
</script>

<template>
  <div v-if="props.table" class="overflow-hidden rounded-xl border border-border bg-card shadow-sm" data-tour="stores-assets">
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead class="px-3">{{ t('stores.assets.currency') }}</TableHead>
          <TableHead class="px-3 text-right">{{ t('stores.assets.amount') }}</TableHead>
          <TableHead class="px-3 text-right">{{ t('stores.table.inCurrency', { sign: props.base.toUpperCase() }) }}</TableHead>
          <TableHead class="px-3 text-right">{{ t('stores.assets.share') }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="row in props.assets" :key="row.asset.symbol">
          <TableCell class="px-3 py-2.5 font-medium">{{ row.asset.symbol.toUpperCase() }}</TableCell>
          <TableCell class="px-3 py-2.5 text-right whitespace-nowrap tabular-nums">
            <Money :amount="row.quantity" :currency="row.asset.symbol" />
          </TableCell>
          <TableCell class="px-3 py-2.5 text-right font-semibold whitespace-nowrap tabular-nums">
            <Money :amount="row.value_base" :currency="props.base" :decimals="0" />
          </TableCell>
          <TableCell class="px-3 py-2.5 text-right text-muted-foreground tabular-nums">{{ pct(row.share) }}</TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
  <div v-else class="flex flex-col gap-2 rounded-xl border border-border bg-card p-4 shadow-sm" data-tour="stores-assets">
    <span class="text-sm font-semibold">{{ t('stores.view.assets') }}</span>
    <div class="flex flex-wrap gap-x-4.5 gap-y-1 text-sm">
      <span v-for="row in props.assets" :key="row.asset.symbol" class="whitespace-nowrap tabular-nums">
        <b class="font-semibold"><Money :amount="row.quantity" :currency="row.asset.symbol" :decimals="0" /></b>
        <span class="text-muted-foreground">
          · <Money :amount="row.value_base" :currency="props.base" :decimals="0" /> · {{ pct(row.share) }}
        </span>
      </span>
    </div>
  </div>
</template>
