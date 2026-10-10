<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { formatMoney } from '@/shared/lib/format'

const props = defineProps<{ report: StoresReport }>()

const { t } = useI18n()
const money = (value: string) => formatMoney(value, { decimals: 0 })
const columns = ['value_start', 'value_end', 'change', 'inflow', 'outflow'] as const
</script>

<template>
  <Card>
    <div class="overflow-x-auto">
      <table class="w-full text-sm tabular-nums">
        <thead class="text-xs text-muted-foreground">
          <tr>
            <th class="py-1 pr-2 text-left font-normal">{{ t('report.stores.wallet') }}</th>
            <th v-for="column in columns" :key="column" class="px-2 py-1 text-right font-normal">
              {{ t(`report.stores.${column}`) }}
            </th>
            <th class="py-1 pl-2 text-right font-normal">{{ t('report.stores.txCount') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in props.report.rows" :key="row.store_id" class="border-t border-border">
            <td class="py-1.5 pr-2">
              <RouterLink
                :to="{ name: 'wallet', params: { id: row.store_id } }"
                class="hover:underline"
                :style="{ paddingLeft: `${row.depth * 1.25}rem` }"
              >
                {{ row.name }}
              </RouterLink>
            </td>
            <td
              v-for="column in columns"
              :key="column"
              class="px-2 py-1.5 text-right"
              :class="column === 'change' && (Number(row.change) < 0 ? 'text-destructive' : Number(row.change) > 0 ? 'text-emerald-600 dark:text-emerald-400' : '')"
            >
              {{ money(row[column]) }}
            </td>
            <td class="py-1.5 pl-2 text-right">{{ row.tx_count }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </Card>
</template>
