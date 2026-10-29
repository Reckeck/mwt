<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { Badge } from '@/shared/ui/badge'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'
import { cn } from '@/shared/lib/utils'

import type { UtilityHistoryRow } from '../../lib/utility-item'

/**
 * `History` of an item (Wallets-2 · 11–12): month, reading (per zone),
 * consumption, amount and a flag — `waiting`, `unusually high`. A phone drops
 * the reading and the flag columns.
 */
const props = defineProps<{ item: UtilityItem; rows: UtilityHistoryRow[]; phone: boolean }>()
const { t } = useI18n()

const meter = computed(() => ('metered' in props.item.kind ? props.item.kind.metered : null))
const zoned = computed(() => Boolean(meter.value?.zones.length))
const readingHead = computed(() => (zoned.value ? meter.value!.zones.join(' / ') : t('utilities.reading')))
const usedHead = computed(() => (zoned.value ? t('utilityTab.consumption') : `${t('utilityTab.consumption')}, ${meter.value?.unit ?? ''}`))
const TH = 'h-8 px-2 text-xs font-medium text-muted-foreground'
const capital = (text: string) => text.charAt(0).toUpperCase() + text.slice(1)
</script>

<template>
  <section class="flex flex-col gap-2" data-utility-history>
    <h3 class="text-sm font-medium">{{ t('utilityItem.history') }}</h3>
    <div class="overflow-hidden rounded-xl border border-border">
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead :class="TH">{{ t('utilityItem.month') }}</TableHead>
          <TableHead v-if="!props.phone" :class="TH">{{ capital(readingHead) }}</TableHead>
          <TableHead :class="TH">{{ usedHead }}</TableHead>
          <TableHead :class="[TH, 'text-right']">{{ t('utilityItem.amount') }}</TableHead>
          <TableHead v-if="!props.phone" class="w-px px-2"><span class="sr-only">{{ t('utilityItem.flag') }}</span></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="row in props.rows" :key="row.period">
          <TableCell class="px-2 py-2">
            {{ row.month }}
            <span v-if="props.phone && (row.waiting || row.anomaly)" class="ms-1 text-xs text-warning">
              · {{ row.waiting ? t('utilityItem.waits') : t('utilityItem.unusual') }}
            </span>
          </TableCell>
          <TableCell v-if="!props.phone" class="px-2 py-2 whitespace-nowrap text-muted-foreground tabular-nums">{{ row.reading }}</TableCell>
          <TableCell :class="cn('px-2 py-2 whitespace-nowrap tabular-nums', row.consumption === '—' && 'text-muted-foreground')">
            {{ row.consumption }}
          </TableCell>
          <TableCell :class="cn('px-2 py-2 text-right whitespace-nowrap tabular-nums', !row.amount && 'text-muted-foreground')">
            <Money v-if="row.amount && 'amount' in row.amount" v-bind="row.amount" />
            <template v-else>—</template>
          </TableCell>
          <TableCell v-if="!props.phone" class="px-2 py-2 text-right">
            <Badge v-if="row.waiting || row.anomaly" variant="outline" class="h-5.5 rounded-md border-warning/40 bg-warning/10 font-normal whitespace-nowrap text-warning">
              {{ row.waiting ? t('utilityItem.waits') : t('utilityItem.unusual') }}
            </Badge>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
    </div>
  </section>
</template>
