<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Money } from '@/entities/currency'

import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { formatMoney, formatWeekday } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { SectionState } from '@/shared/ui/section-state'

const { t } = useI18n()

const today = new Date()
const thisMonth = localDate(today).slice(0, 7)
const weekAgo = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 6)
const lastMonth = localDate(weekAgo).slice(0, 7)

/** The last seven days can reach into the previous month. */
const current = useQuery({
  queryKey: ['calendar', thisMonth],
  queryFn: () => call<CalendarMonth>('get_calendar', { month: thisMonth }),
})
const previous = useQuery({
  queryKey: ['calendar', lastMonth],
  queryFn: () => call<CalendarMonth>('get_calendar', { month: lastMonth }),
  enabled: lastMonth !== thisMonth,
})

const days = computed(() => {
  const all = [...(previous.data.value?.days ?? []), ...(current.data.value?.days ?? [])]
  return Array.from({ length: 7 }, (_unused, index) => {
    const date = localDate(new Date(weekAgo.getFullYear(), weekAgo.getMonth(), weekAgo.getDate() + index))
    const day = all.find((entry) => entry.date === date)
    return { date, daily: Number(day?.daily ?? 0), status: day?.status ?? 'none' }
  })
})
const max = computed(() => Math.max(1, ...days.value.map((day) => day.daily), Number(current.data.value?.allowance ?? 0)))
const total = computed(() => days.value.reduce((sum, day) => sum + day.daily, 0))
const weekday = (date: string) => formatWeekday(date, 'narrow')
</script>

<template>
  <Card size="sm" v-if="current.data.value">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <RouterLink data-widget-title :to="{ name: 'transactions-calendar' }" class="text-sm text-muted-foreground hover:text-foreground">
          {{ t('spending7d.title') }}
        </RouterLink>
        <span class="text-sm tabular-nums">
          <Money :amount="total" :decimals="0" :currency="current.data.value.currency" />
        </span>
      </div>
    </template>
    <div class="flex h-28 items-end gap-2" role="img" :aria-label="t('spending7d.title')">
      <div v-for="day in days" :key="day.date" class="flex flex-1 flex-col items-center gap-1">
        <div class="flex h-24 w-full items-end">
          <div
            class="w-full rounded-t-sm"
            :class="day.status === 'over' ? 'bg-destructive' : day.status === 'near' ? 'bg-warning' : 'bg-primary'"
            :style="{ height: `${Math.max(2, (day.daily / max) * 100)}%` }"
            :title="formatMoney(day.daily, { decimals: 0 })"
          />
        </div>
        <span class="text-2xs text-muted-foreground uppercase">{{ weekday(day.date) }}</span>
      </div>
    </div>
  </Card>
  <!-- Holds the cell while loading, so the widget does not pop in. -->
  <!-- A failed load says so and offers a retry; it used to stay a skeleton for ever. -->
  <Card size="sm" v-else-if="current.error.value">
    <SectionState :error="current.error.value" compact @retry="current.refetch()" />
  </Card>
  <Card size="sm" v-else><Skeleton class="h-24 w-full" /></Card>
</template>
