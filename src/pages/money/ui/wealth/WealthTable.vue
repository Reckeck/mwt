<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatMoney } from '@/shared/lib/format'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { wealthRows } from '@/entities/report'

/** «Из чего»: each group's value and share now (Money 2 · 12); a card list on a phone. */
const props = defineProps<{ point: NetworthPoint | undefined; currency: string }>()

const { t } = useI18n()
const rows = computed(() => wealthRows(props.point))
const money = (value: number) => formatMoney(value, { currency: props.currency })
</script>

<template>
  <div v-if="rows.length" class="rounded-xl border bg-card shadow-sm" data-tour="wealth-table">
    <Table class="max-md:hidden">
      <TableHeader>
        <TableRow>
          <TableHead>{{ t('money.wealth.of') }}</TableHead>
          <TableHead class="text-right">{{ t('money.wealth.sum') }}</TableHead>
          <TableHead class="w-32 text-right">{{ t('money.wealth.share') }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="row in rows" :key="row.group">
          <TableCell>{{ t(`money.wealth.group.${row.group}`) }}</TableCell>
          <TableCell class="text-right font-semibold tabular-nums" :class="row.value < 0 && 'text-destructive'">{{ money(row.value) }}</TableCell>
          <TableCell class="text-right tabular-nums text-muted-foreground">{{ row.share === null ? '' : `${row.share} %` }}</TableCell>
        </TableRow>
      </TableBody>
    </Table>
    <ul class="divide-y md:hidden">
      <li v-for="row in rows" :key="row.group" class="flex items-baseline justify-between gap-3 px-4 py-3 text-sm">
        <span class="min-w-0">
          {{ t(`money.wealth.group.${row.group}`) }}
          <span v-if="row.share !== null" class="text-muted-foreground"> · {{ row.share }} %</span>
        </span>
        <b class="shrink-0 tabular-nums" :class="row.value < 0 && 'text-destructive'">{{ money(row.value) }}</b>
      </li>
    </ul>
  </div>
</template>
