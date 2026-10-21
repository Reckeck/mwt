<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { formatDate } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

/** Year by year, each opening into its months. */
const props = defineProps<{ years: InvestYear[]; currency: string }>()

const { t } = useI18n()
const opened = ref<number[]>([])
const rows = computed(() => props.years.map((row) => ({ ...row, open: opened.value.includes(row.year) })))

function toggle(year: number) {
  opened.value = opened.value.includes(year) ? opened.value.filter((entry) => entry !== year) : [...opened.value, year]
}
</script>

<template>
  <div class="overflow-x-auto [scrollbar-gutter:stable]" data-tour="calc-table">
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{{ t('invest.col.period') }}</TableHead>
          <TableHead class="text-right">{{ t('invest.col.contributed') }}</TableHead>
          <TableHead class="text-right">{{ t('invest.col.interest') }}</TableHead>
          <TableHead class="text-right">{{ t('invest.col.tax') }}</TableHead>
          <TableHead class="text-right">{{ t('invest.col.withdrawn') }}</TableHead>
          <TableHead class="text-right">{{ t('invest.col.balance') }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <template v-for="row in rows" :key="row.year">
          <TableRow>
            <TableCell class="whitespace-nowrap">
              <Button
                variant="ghost"
                size="sm"
                class="-ml-2"
                :aria-expanded="row.open"
                @click="toggle(row.year)"
              >
                <ChevronRight class="size-4 transition-[rotate]" :class="row.open && 'rotate-90'" />
                {{ t('invest.yearN', { year: row.year }) }}
              </Button>
            </TableCell>
            <TableCell class="text-right"><Money :amount="row.contributed" :currency="props.currency" :decimals="0" /></TableCell>
            <TableCell class="text-right"><Money :amount="row.interest" :currency="props.currency" :decimals="0" /></TableCell>
            <TableCell class="text-right"><Money :amount="row.tax" :currency="props.currency" :decimals="0" /></TableCell>
            <TableCell class="text-right"><Money :amount="row.withdrawn" :currency="props.currency" :decimals="0" /></TableCell>
            <TableCell class="text-right font-medium"><Money :amount="row.balance" :currency="props.currency" :decimals="0" /></TableCell>
          </TableRow>
          <template v-if="row.open">
            <TableRow v-for="month in row.months" :key="month.month" class="text-muted-foreground">
              <TableCell class="pl-8 whitespace-nowrap">{{ formatDate(month.date) }}</TableCell>
              <TableCell class="text-right"><Money :amount="month.contributed" :currency="props.currency" :decimals="0" /></TableCell>
              <TableCell class="text-right"><Money :amount="month.interest" :currency="props.currency" :decimals="0" /></TableCell>
              <TableCell class="text-right"><Money :amount="month.tax" :currency="props.currency" :decimals="0" /></TableCell>
              <TableCell class="text-right"><Money :amount="month.withdrawn" :currency="props.currency" :decimals="0" /></TableCell>
              <TableCell class="text-right"><Money :amount="month.balance" :currency="props.currency" :decimals="0" /></TableCell>
            </TableRow>
          </template>
        </template>
      </TableBody>
    </Table>
  </div>
</template>
