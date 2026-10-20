<script setup lang="ts" generic="TData">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-vue-next'
import {
  FlexRender,
  type Column,
  type SortingState,
  getCoreRowModel,
  getSortedRowModel,
  useVueTable,
} from '@tanstack/vue-table'

import { cn } from '@/shared/lib/utils'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import type { DataTableProps } from './data-table.types'

/**
 * The app's table: shadcn `Table` + TanStack Table. Click a header to sort
 * (again to flip, a third time to reset); numeric columns align right with
 * tabular digits (`meta.align`); thin row dividers, no zebra stripes.
 */
const props = withDefaults(defineProps<DataTableProps<TData>>(), {
  initialSorting: () => [],
  getRowId: undefined,
  stickyHeader: false,
  stickyFirstColumn: false,
  emptyText: undefined,
  clickable: false,
})
const emit = defineEmits<{ 'row-click': [row: TData] }>()

/** The pinned first cell covers what scrolls under it; the header row stays above. */
const STICKY_CELL = 'sticky left-0 z-[1] bg-card'

const { t } = useI18n()

const sorting = ref<SortingState>(props.initialSorting)

const table = useVueTable({
  get data() {
    return props.data
  },
  get columns() {
    return props.columns
  },
  getRowId: props.getRowId,
  state: {
    get sorting() {
      return sorting.value
    },
  },
  onSortingChange: (updater) => {
    sorting.value = typeof updater === 'function' ? updater(sorting.value) : updater
  },
  getCoreRowModel: getCoreRowModel(),
  getSortedRowModel: getSortedRowModel(),
})

const ALIGN = { left: 'text-left', right: 'text-right', center: 'text-center' } as const

function alignOf(column: Column<TData, unknown>): string {
  return ALIGN[column.columnDef.meta?.align ?? 'left']
}

function ariaSort(column: Column<TData, unknown>): 'ascending' | 'descending' | undefined {
  const dir = column.getIsSorted()
  if (!dir) return undefined
  return dir === 'asc' ? 'ascending' : 'descending'
}

function headerLabel(column: Column<TData, unknown>): string {
  const header = column.columnDef.header
  return typeof header === 'string' ? header : column.id
}
</script>

<template>
  <Table :container-class="props.containerClass">
    <TableHeader :class="cn(props.stickyHeader && 'sticky top-0 z-10 bg-card')">
      <TableRow v-for="group in table.getHeaderGroups()" :key="group.id" class="hover:bg-transparent">
        <TableHead
          v-for="(header, index) in group.headers"
          :key="header.id"
          :aria-sort="ariaSort(header.column)"
          :class="
            cn(
              'text-xs font-medium text-muted-foreground',
              alignOf(header.column),
              header.column.columnDef.meta?.class,
              props.stickyFirstColumn && index === 0 && STICKY_CELL,
            )
          "
        >
          <template v-if="!header.isPlaceholder">
            <button
              v-if="header.column.getCanSort()"
              type="button"
              :class="
                cn(
                  'touch-hit-xl inline-flex items-center gap-1 rounded-sm hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
                  header.column.columnDef.meta?.align === 'right' && 'flex-row-reverse',
                )
              "
              :aria-label="t('ui.sortBy', { column: headerLabel(header.column) })"
              @click="header.column.toggleSorting(undefined, false)"
            >
              <FlexRender :render="header.column.columnDef.header" :props="header.getContext()" />
              <ArrowUp v-if="header.column.getIsSorted() === 'asc'" class="size-3.5" />
              <ArrowDown v-else-if="header.column.getIsSorted() === 'desc'" class="size-3.5" />
              <ArrowUpDown v-else class="size-3.5 opacity-40" />
            </button>
            <FlexRender v-else :render="header.column.columnDef.header" :props="header.getContext()" />
          </template>
        </TableHead>
      </TableRow>
    </TableHeader>
    <TableBody>
      <template v-if="table.getRowModel().rows.length">
        <TableRow
          v-for="row in table.getRowModel().rows"
          :key="row.id"
          :class="cn(props.clickable && 'cursor-pointer focus-visible:bg-muted/50 focus-visible:outline-none')"
          :tabindex="props.clickable ? 0 : undefined"
          @click="props.clickable && emit('row-click', row.original)"
          @keydown.enter="props.clickable && emit('row-click', row.original)"
        >
          <TableCell
            v-for="(cell, index) in row.getVisibleCells()"
            :key="cell.id"
            :class="
              cn(
                alignOf(cell.column),
                cell.column.columnDef.meta?.align === 'right' && 'tabular-nums',
                cell.column.columnDef.meta?.class,
                props.stickyFirstColumn && index === 0 && STICKY_CELL,
              )
            "
          >
            <FlexRender :render="cell.column.columnDef.cell" :props="cell.getContext()" />
          </TableCell>
        </TableRow>
      </template>
      <TableRow v-else class="hover:bg-transparent">
        <TableCell :colspan="props.columns.length" class="h-20 text-center text-sm text-muted-foreground">
          {{ props.emptyText ?? t('ui.empty') }}
        </TableCell>
      </TableRow>
    </TableBody>
  </Table>
</template>
