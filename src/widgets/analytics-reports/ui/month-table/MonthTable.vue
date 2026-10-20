<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { ChevronDown } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { ReportTerm } from '@/entities/report'
import { Button } from '@/shared/ui/button'
import { signClass } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import type { MonthTableColumn, MonthTableProps, MonthTableRow } from './month-table.types'

/**
 * Months down, figures across. On a narrow screen only the primary columns
 * stay; a row's other figures open under it.
 */
const props = defineProps<MonthTableProps>()

const { t } = useI18n()
const opened = ref(new Set<string>())
const secondary = computed(() => props.columns.filter((column) => !column.primary))

/** Text for a muted row or a column with its own format; `null` draws the sum as `<Money>`. */
function cell(column: MonthTableColumn, row: MonthTableRow): string | null {
  if (row.muted || row.values[column.key] === '') return '—'
  return column.format ? column.format(row.values[column.key] ?? '0') : null
}

function toggle(id: string) {
  const next = new Set(opened.value)
  if (!next.delete(id)) next.add(id)
  opened.value = next
}
</script>

<template>
  <div class="overflow-x-auto">
    <Table class="w-full text-sm tabular-nums">
      <TableHeader>
        <TableRow class="hover:bg-transparent">
          <TableHead class="text-left sticky left-0 z-[1] bg-card">{{ props.labelHeader }}</TableHead>
          <TableHead
            v-for="column in props.columns"
            :key="column.key"
            :class="cn('text-right', !column.primary && 'hidden @2xl:table-cell')"
          >
            <ReportTerm :label="column.label" :hint="column.hint" />
          </TableHead>
          <TableHead v-if="secondary.length" class="w-8 @2xl:hidden"><span class="sr-only">{{ t('report.more') }}</span></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <template v-for="row in props.rows" :key="row.id">
          <TableRow :class="cn(row.muted && 'text-muted-foreground', row.bold && 'font-medium')">
            <TableCell class="capitalize sticky left-0 z-[1] bg-card">
              <RouterLink v-if="row.to && !row.muted" :to="row.to" class="hover:underline" :title="t('report.openTransactions')">
                {{ row.label }}
              </RouterLink>
              <template v-else>{{ row.label }}</template>
            </TableCell>
            <TableCell
              v-for="column in props.columns"
              :key="column.key"
              :class="cn('text-right', !column.primary && 'hidden @2xl:table-cell', column.signed && !row.muted && signClass(row.values[column.key]))"
            >
              <template v-if="cell(column, row) !== null">{{ cell(column, row) }}</template>
              <Money v-else :amount="row.values[column.key] ?? '0'" :currency="props.currency" :decimals="0" />
            </TableCell>
            <TableCell v-if="secondary.length" class="w-8 p-0 @2xl:hidden">
              <Button
                variant="ghost"
                size="icon-xs"
                :aria-expanded="opened.has(row.id)"
                :aria-label="t('report.more')"
                @click="toggle(row.id)"
              >
                <ChevronDown :class="cn('size-4 transition-transform', opened.has(row.id) && 'rotate-180')" />
              </Button>
            </TableCell>
          </TableRow>
          <TableRow v-if="opened.has(row.id)" class="bg-muted/40 hover:bg-muted/40 @2xl:hidden">
            <TableCell :colspan="props.columns.length + 2">
              <dl class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-1 text-xs">
                <template v-for="column in secondary" :key="column.key">
                  <dt class="text-muted-foreground">{{ column.label }}</dt>
                  <dd :class="cn('text-right', column.signed && !row.muted && signClass(row.values[column.key]))">
                    <template v-if="cell(column, row) !== null">{{ cell(column, row) }}</template>
              <Money v-else :amount="row.values[column.key] ?? '0'" :currency="props.currency" :decimals="0" />
                  </dd>
                </template>
              </dl>
            </TableCell>
          </TableRow>
        </template>
      </TableBody>
    </Table>
  </div>
</template>
