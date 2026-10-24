<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'
import { formatMonthKey } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import { phoneSubtitle, utilityLineView } from '../../lib/utility-line'

/**
 * The items of the bill (Wallets-2 · 10): a table on a desktop — item, reading,
 * consumption, this month's amount, «Внести» or «авто» — and a list on a phone
 * with the columns folded into the line under the name.
 */
const props = defineProps<{ bill: UtilityBill; phone: boolean; disabled: boolean; active: string | null }>()
const emit = defineEmits<{ open: [line: UtilityBillLine] }>()
const { t } = useI18n()

const rows = computed(() => props.bill.lines.map((line) => utilityLineView(line, t)))
const month = computed(() => formatMonthKey(props.bill.period, { month: 'long' }))
const ROW = 'flex h-auto min-h-13 w-full items-center justify-start gap-3 rounded-none px-3 py-1.5 text-left font-normal whitespace-normal'
const TH = 'h-9 px-3 text-xs font-medium text-muted-foreground'
</script>

<template>
  <div class="overflow-hidden rounded-xl border border-border bg-card shadow-sm" :class="props.phone && 'px-3'" data-utility-lines>
    <ul v-if="props.phone" class="divide-y divide-border">
      <li v-for="row in rows" :key="row.line.item.id" class="-mx-3">
        <Button
          variant="ghost"
          :class="cn(ROW, props.active === row.line.item.id && 'bg-primary/10')"
          @click="emit('open', row.line)"
        >
        <span class="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-muted text-muted-foreground">
          <component :is="row.icon" class="size-4" />
        </span>
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="truncate font-medium">{{ row.name }}</span>
          <span class="truncate text-xs text-muted-foreground">{{ phoneSubtitle(row, t) }}</span>
        </span>
        <span :class="cn('font-medium whitespace-nowrap tabular-nums', row.waiting && 'text-warning', row.automated && 'text-muted-foreground')">
          {{ row.amount }}
        </span>
        </Button>
      </li>
    </ul>
    <Table v-else>
      <TableHeader>
        <TableRow>
          <TableHead :class="TH">{{ t('utilityTab.item') }}</TableHead>
          <TableHead :class="TH">{{ t('utilityTab.reading') }}</TableHead>
          <TableHead :class="TH">{{ t('utilityTab.consumption') }}</TableHead>
          <TableHead :class="[TH, 'text-right']">{{ t('utilityTab.forMonth', { month }) }}</TableHead>
          <TableHead class="w-px px-3"><span class="sr-only">{{ t('bonuses.actions') }}</span></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow
          v-for="row in rows"
          :key="row.line.item.id"
          :data-utility-line="row.line.item.id"
          :class="cn('cursor-pointer', props.active === row.line.item.id && 'bg-primary/10 hover:bg-primary/10')"
          @click="emit('open', row.line)"
        >
          <TableCell class="px-3 py-2.5">
            <div class="flex items-center gap-2">
              <component :is="row.icon" class="size-4 shrink-0 text-muted-foreground" />
              <span class="flex min-w-0 flex-col">
                <span class="truncate font-medium">{{ row.name }}</span>
                <span class="truncate text-xs text-muted-foreground">{{ row.subtitle }}</span>
              </span>
            </div>
          </TableCell>
          <TableCell class="px-3 py-2.5 whitespace-nowrap text-muted-foreground tabular-nums">{{ row.reading }}</TableCell>
          <TableCell :class="cn('px-3 py-2.5 whitespace-nowrap', row.waiting && 'text-warning', row.consumption === '—' && 'text-muted-foreground')">
            {{ row.consumption }}
          </TableCell>
          <TableCell class="px-3 py-2.5 text-right font-medium whitespace-nowrap tabular-nums">{{ row.amount }}</TableCell>
          <TableCell class="px-3 py-2.5 text-right">
            <Button v-if="row.waiting" variant="outline" size="sm" :disabled="props.disabled" @click.stop="emit('open', row.line)">
              {{ t('utilityTab.read') }}
            </Button>
            <Badge v-else-if="row.automated" variant="outline" class="h-5.5 rounded-md font-normal">{{ t('utilityTab.auto') }}</Badge>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
</template>
