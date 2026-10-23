<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute } from 'vue-router'
import { ChevronDown, ChevronRight } from 'lucide-vue-next'

import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { drillTo } from '@/shared/lib/drill'
import { formatMoney, pluralForm } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { WHERE_OPENS, WHERE_TX_KINDS, exchangeText, sharedUnit, unitText } from '../../lib/where-view'
import WhereOpened from './WhereOpened.vue'

/**
 * Where the money went (Money 1 · 6): one line per direction — spent, lent,
 * moved between one's own wallets, invested, lost — what it was and how much.
 * On a phone the lines stack in one card. A row of operations opens on the
 * spot (Paths 2 · 9) and closes on a second press.
 */
const props = defineProps<{ report: WhereReport; filter: ReportFilter }>()

const { t, locale } = useI18n()
const route = useRoute()
const opened = ref<WhereKind | null>(null)
const toggle = (kind: WhereKind) => (opened.value = opened.value === kind ? null : kind)
/** The row's operations in the list, with «← Куда ушли» back here. */
const listOf = (kind: WhereKind) =>
  drillTo({
    from: props.filter.from ?? undefined,
    to: props.filter.to ?? undefined,
    storeIds: props.filter.scope.store_ids,
    kinds: WHERE_TX_KINDS[kind],
    back: { to: route.fullPath, label: t('money.where.back') },
  })
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
  props.report.rows.map((row) => ({
    row,
    kind: row.kind,
    title: t(`money.where.kind.${row.kind}`),
    what: what(row),
    much: howMuch(row),
    tone: TONE[row.kind],
    opens: WHERE_OPENS.includes(row.kind) && row.items.some((item) => item.at),
    open: opened.value === row.kind,
    to: listOf(row.kind),
    toggle: () => toggle(row.kind),
  })),
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
          <template v-for="line in lines" :key="line.kind">
            <TableRow
              :class="[line.opens && 'cursor-pointer', line.open && 'bg-primary/5 hover:bg-primary/5']"
              :data-where-row="line.kind"
              @click="line.opens && line.toggle()"
            >
              <TableCell class="px-3 font-medium">
                <Button
                  v-if="line.opens"
                  variant="ghost"
                  size="sm"
                  class="-mx-2 h-auto gap-1 px-2 py-1 font-medium"
                  :aria-expanded="line.open"
                  :aria-label="t('money.where.expand', { kind: line.title })"
                  @click.stop="line.toggle()"
                >
                  <ChevronRight :class="cn('size-3.5 text-muted-foreground transition-transform', line.open && 'rotate-90')" />
                  {{ line.title }}
                </Button>
                <template v-else>{{ line.title }}</template>
              </TableCell>
              <TableCell class="max-w-0 truncate px-3 text-xs text-muted-foreground" :title="line.what">
                {{ line.open ? t(`money.where.hint.${line.kind}`) : line.what }}
              </TableCell>
              <TableCell :class="['px-3 text-right font-semibold whitespace-nowrap tabular-nums', line.tone]">{{ line.much }}</TableCell>
            </TableRow>
            <WhereOpened v-if="line.open" :row="line.row" :to="line.to" />
          </template>
        </TableBody>
      </Table>
    </div>
    <div class="divide-y rounded-xl border bg-card px-3 shadow-sm md:hidden">
      <div
        v-for="line in lines"
        :key="line.kind"
        class="py-2.5"
        :class="line.open && '-mx-3 bg-primary/5 px-3'"
        :data-where-row="line.kind"
      >
        <Button
          v-if="line.opens"
          variant="ghost"
          size="block"
          class="flex h-auto w-full items-center gap-3 p-0 text-left hover:bg-transparent"
          :aria-expanded="line.open"
          @click="line.toggle()"
        >
          <span class="grid min-w-0 flex-1">
            <span class="text-sm font-medium">{{ line.title }}</span>
            <span class="truncate text-xs font-normal text-muted-foreground">
              {{ line.open ? t(`money.where.hintShort.${line.kind}`) : line.what }}
            </span>
          </span>
          <b :class="['text-sm whitespace-nowrap tabular-nums', line.tone]">{{ line.much }}</b>
          <ChevronDown class="size-4 shrink-0 text-muted-foreground transition-transform" :class="line.open && 'rotate-180'" />
        </Button>
        <div v-else class="flex items-center gap-3">
          <div class="grid min-w-0 flex-1">
            <span class="text-sm font-medium">{{ line.title }}</span>
            <span class="truncate text-xs text-muted-foreground">{{ line.what }}</span>
          </div>
          <b :class="['text-sm whitespace-nowrap tabular-nums', line.tone]">{{ line.much }}</b>
        </div>
        <WhereOpened v-if="line.open" :row="line.row" :to="line.to" phone />
      </div>
    </div>
  </section>
</template>
