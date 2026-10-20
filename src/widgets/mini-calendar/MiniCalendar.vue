<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { STATUS_CLASSES } from '@/widgets/spending-calendar'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { localDate } from '@/shared/lib/planning'
import { formatMonthKey } from '@/shared/lib/format'
import { SectionState } from '@/shared/ui/section-state'

const { t } = useI18n()

const month = localDate(new Date()).slice(0, 7)
const calendar = useQuery({
  queryKey: ['calendar', month],
  queryFn: () => call<CalendarMonth>('get_calendar', { month }),
})
const lead = (new Date(`${month}-01T12:00`).getDay() + 6) % 7
const title = formatMonthKey(month, { month: 'long' })
const upcomingDates = computed(() => new Set((calendar.data.value?.upcoming ?? []).map((item) => item.date)))
</script>

<template>
  <Card size="sm" v-if="calendar.data.value">
    <template #header>
      <RouterLink data-widget-title :to="{ name: 'transactions-calendar' }" class="text-sm text-muted-foreground capitalize hover:text-foreground">
        {{ t('miniCalendar.title', { month: title }) }}
      </RouterLink>
    </template>
    <div class="grid grid-cols-7 gap-1 text-center text-2xs tabular-nums">
      <div v-for="index in lead" :key="`lead-${index}`" />
      <div
        v-for="day in calendar.data.value.days"
        :key="day.date"
        class="relative rounded-sm border py-1"
        :class="STATUS_CLASSES[day.status]"
        :title="day.daily !== '0' ? day.daily : undefined"
      >
        {{ Number(day.date.slice(8)) }}
        <span v-if="upcomingDates.has(day.date)" class="absolute right-0.5 top-0.5 size-1.5 rounded-full bg-primary" />
      </div>
    </div>
  </Card>
  <!-- Holds the cell while loading, so the widget does not pop in. -->
  <!-- A failed load says so and offers a retry; it used to stay a skeleton for ever. -->
  <Card size="sm" v-else-if="calendar.error.value">
    <SectionState :error="calendar.error.value" compact @retry="calendar.refetch()" />
  </Card>
  <Card size="sm" v-else><Skeleton class="h-24 w-full" /></Card>
</template>
