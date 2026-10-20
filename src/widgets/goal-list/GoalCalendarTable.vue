<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'
import { call } from '@/shared/api/ipc'
import { formatMoney, formatMonthKey } from '@/shared/lib/format'

const { t } = useI18n()

const year = ref(new Date().getFullYear())
const calendar = useQuery({
  queryKey: computed(() => ['goal-calendar', year.value]),
  placeholderData: keepPreviousData,
  queryFn: () => call<GoalCalendar>('get_goal_calendar', { year: year.value }),
})

const monthName = (key: string) => formatMonthKey(key, { month: 'short' })
const money = (value: string) => formatMoney(value, { currency: calendar.data.value?.currency })
const isFuture = (key: string) => key > new Date().toISOString().slice(0, 7)
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <h3 class="heading-card">{{ t('goalCalendar.title') }}</h3>
        <div class="flex items-center gap-1">
          <Button variant="ghost" size="icon-sm"  :aria-label="t('goalCalendar.prev')" @click="year -= 1">
            <ChevronLeft class="size-4" />
          </Button>
          <span class="w-12 text-center text-sm tabular-nums">{{ year }}</span>
          <Button variant="ghost" size="icon-sm"  :aria-label="t('goalCalendar.next')" @click="year += 1">
            <ChevronRight class="size-4" />
          </Button>
        </div>
      </div>
    </template>
    <div class="overflow-x-auto">
      <!-- Months in calendar order: nothing to sort, so the plain shadcn Table. -->
      <Table class="tabular-nums">
        <TableHeader>
          <TableRow class="hover:bg-transparent">
            <TableHead>{{ t('goalCalendar.month') }}</TableHead>
            <TableHead class="text-right">{{ t('goalCalendar.need') }}</TableHead>
            <TableHead class="text-right">{{ t('goalCalendar.saved') }}</TableHead>
            <TableHead class="text-right">{{ t('goalCalendar.status') }}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-for="month in calendar.data.value?.months ?? []" :key="month.month">
            <TableCell class="capitalize">{{ monthName(month.month) }}</TableCell>
            <TableCell class="text-right"><Money :amount="month.need" :currency="calendar.data.value?.currency" /></TableCell>
            <TableCell class="text-right"><Money :amount="month.saved" :currency="calendar.data.value?.currency" /></TableCell>
            <TableCell class="text-right text-xs">
              <span v-if="Number(month.need) === 0" class="text-muted-foreground">—</span>
              <span v-else-if="month.done" class="text-success">✓ {{ t('goalCalendar.done') }}</span>
              <span v-else-if="isFuture(month.month)" class="text-muted-foreground">{{ t('goalCalendar.ahead') }}</span>
              <span v-else class="text-warning">{{ t('goalCalendar.short', { amount: money(month.shortfall) }) }}</span>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
    </div>
  </Card>
</template>
