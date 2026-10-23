<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney, pluralForm } from '@/shared/lib/format'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { exchangeText, sharedUnit, unitText } from '../../lib/where-view'

/**
 * Where the money went (Money 1 · 6): one line per direction — spent, lent,
 * moved between one's own wallets, invested, lost — what it was and how much.
 * On a phone the lines stack in one card.
 */
const props = defineProps<{ report: WhereReport }>()

const { t, locale } = useI18n()
const categories = useCategories()
const money = (value: string) => formatMoney(value, { decimals: Math.abs(Number(value)) < 100 ? 2 : 0, currency: props.report.meta.currency })

const TONE: Record<WhereKind, string> = {
  spending: 'text-destructive',
  lent: '',
  moved: 'text-muted-foreground',
  invested: '',
  losses: 'text-warning',
}

function what(row: WhereRow): string {
  const more = row.count > row.items.length && row.kind !== 'spending' ? [t('money.where.more', { count: row.count - row.items.length })] : []
  switch (row.kind) {
    case 'spending':
      return row.items.map((item) => categoryKeyLabel(item.label, categories.data.value, t) || t('money.categories.none')).join(', ')
    case 'lent':
      return [...row.items.map((item) => [item.comment || item.label, item.to].filter(Boolean).join(', ')), ...more].join(' · ')
    case 'moved':
      return [...row.items.map((item) => `${item.label} → ${item.to ?? ''} ${exchangeText(item)}`), ...more].join(' · ')
    case 'invested':
      return [...row.items.map((item) => t('money.where.spot', { trade: exchangeText(item) })), ...more].join(' · ')
    case 'losses':
      return t('money.where.fees')
  }
}

function howMuch(row: WhereRow): string {
  const unit = sharedUnit(row)
  const own = (qty: (item: WhereItem) => string) => unitText(String(row.items.reduce((sum, item) => sum + Math.abs(Number(qty(item))), 0)), unit ?? '')
  switch (row.kind) {
    case 'moved':
      return t(`money.where.transfers.${pluralForm(row.count, locale.value)}`, { count: row.count })
    case 'lent':
      return unit && unit.toLowerCase() !== props.report.meta.currency ? `${own((item) => item.qty)} · ${money(row.value)}` : money(row.value)
    case 'invested':
      return unit && row.count === row.items.length ? own((item) => item.qty) : money(row.value)
    default:
      return money(row.value)
  }
}

const lines = computed(() =>
  props.report.rows.map((row) => ({ kind: row.kind, title: t(`money.where.kind.${row.kind}`), what: what(row), much: howMuch(row), tone: TONE[row.kind] })),
)
</script>

<template>
  <section data-tour="where-table">
    <div class="overflow-hidden rounded-xl border bg-card shadow-sm max-md:hidden">
      <Table>
        <TableHeader>
          <TableRow class="text-xs">
            <TableHead class="w-36 px-3 text-muted-foreground">{{ t('money.where.where') }}</TableHead>
            <TableHead class="px-3 text-muted-foreground">{{ t('money.where.what') }}</TableHead>
            <TableHead class="px-3 text-right text-muted-foreground">{{ t('money.where.much') }}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="line in lines" :key="line.kind">
            <TableCell class="px-3 font-medium">{{ line.title }}</TableCell>
            <TableCell class="max-w-0 truncate px-3 text-xs text-muted-foreground" :title="line.what">{{ line.what }}</TableCell>
            <TableCell :class="['px-3 text-right font-semibold whitespace-nowrap tabular-nums', line.tone]">{{ line.much }}</TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
    <div class="divide-y rounded-xl border bg-card px-3 shadow-sm md:hidden">
      <div v-for="line in lines" :key="line.kind" class="flex items-center gap-3 py-2.5">
        <div class="grid min-w-0 flex-1">
          <span class="text-sm font-medium">{{ line.title }}</span>
          <span class="truncate text-xs text-muted-foreground">{{ line.what }}</span>
        </div>
        <b :class="['text-sm whitespace-nowrap tabular-nums', line.tone]">{{ line.much }}</b>
      </div>
    </div>
  </section>
</template>
