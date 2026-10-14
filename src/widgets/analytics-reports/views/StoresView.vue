<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { formatMoney, signClass } from '@/shared/lib/format'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

const props = defineProps<{ report: StoresReport }>()

const { t } = useI18n()
const money = (value: string) => formatMoney(value, { decimals: 0 })
const columns = ['value_start', 'value_end', 'change', 'inflow', 'outflow'] as const
</script>

<template>
  <Card>
    <div class="overflow-x-auto">
      <Table class="w-full text-sm tabular-nums">
        <TableHeader>
          <TableRow class="hover:bg-transparent">
            <TableHead class="text-left">{{ t('report.stores.wallet') }}</TableHead>
            <TableHead v-for="column in columns" :key="column" class="text-right">
              {{ t(`report.stores.${column}`) }}
            </TableHead>
            <TableHead class="text-right">{{ t('report.stores.txCount') }}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="row in props.report.rows" :key="row.store_id">
            <TableCell>
              <RouterLink
                :to="{ name: 'wallet', params: { id: row.store_id } }"
                class="hover:underline"
                :style="{ paddingLeft: `${row.depth * 1.25}rem` }"
              >
                {{ row.name }}
              </RouterLink>
            </TableCell>
            <TableCell
              v-for="column in columns"
              :key="column"
              class="text-right"
              :class="column === 'change' && signClass(row.change)"
            >
              {{ money(row[column]) }}
            </TableCell>
            <TableCell class="text-right">{{ row.tx_count }}</TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
  </Card>
</template>
