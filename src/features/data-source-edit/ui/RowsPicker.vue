<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

/** The first rows of a CSV or XLSX file; a header picks the column. */
const props = defineProps<{ rows: string[][]; selected: string }>()
const emit = defineEmits<{ pick: [column: string] }>()
const { t } = useI18n()
</script>

<template>
  <div class="grid gap-1.5">
    <p class="text-xs text-muted-foreground">{{ t('sources.picker.rowsHint') }}</p>
    <div class="h-80 overflow-auto rounded-md border [scrollbar-gutter:stable] md:h-96" data-tour="source-picker">
      <Table v-if="props.rows.length">
        <TableHeader>
          <TableRow>
            <TableHead v-for="(cell, index) in props.rows[0]" :key="index">
              <Button
                variant="ghost"
                size="sm"
                :class="props.selected === cell && 'bg-accent text-accent-foreground'"
                @click="emit('pick', cell)"
              >
                {{ cell || '—' }}
              </Button>
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="(row, rowIndex) in props.rows.slice(1, 30)" :key="rowIndex">
            <TableCell v-for="(cell, index) in row" :key="index" class="text-xs">{{ cell }}</TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
  </div>
</template>
