<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { STATUS_CLASSES } from '@/widgets/spending-calendar'
import { call } from '@/shared/api/ipc'
import { drillTo } from '@/shared/lib/drill'
import { formatMonthKey } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { Button } from '@/shared/ui/button'

/** The month's days with their spending written in; a day opens its operations. */
const { t } = useI18n()

const thisMonth = localDate(new Date()).slice(0, 7)
const month = ref(thisMonth)
function shift(step: -1 | 1) {
  const [year, number] = month.value.split('-').map(Number)
  month.value = localDate(new Date(year, number - 1 + step, 1)).slice(0, 7)
}

const calendar = useQuery({
  queryKey: computed(() => ['calendar', month.value]),
  queryFn: () => call<CalendarMonth>('get_calendar', { month: month.value }),
  placeholderData: keepPreviousData,
})
const lead = computed(() => (new Date(`${month.value}-01T12:00`).getDay() + 6) % 7)
</script>

<template>
  <div class="flex flex-col gap-1">
    <div class="flex items-center justify-between gap-1">
      <Button variant="ghost" size="icon-sm" :aria-label="t('dashboard.spending.prevMonth')" @click="shift(-1)">
        <ChevronLeft class="size-4" />
      </Button>
      <span class="text-xs capitalize">{{ formatMonthKey(month, { month: 'long' }) }}</span>
      <!-- Kept in place when hidden: the month title must not jump. -->
      <Button
        variant="ghost"
        size="icon-sm"
        :class="month === thisMonth && 'invisible'"
        :aria-hidden="month === thisMonth"
        :tabindex="month === thisMonth ? -1 : undefined"
        :aria-label="t('dashboard.spending.nextMonth')"
        @click="shift(1)"
      >
        <ChevronRight class="size-4" />
      </Button>
    </div>
    <div class="grid grid-cols-7 gap-0.5 text-center text-2xs tabular-nums">
      <div v-for="index in lead" :key="`lead-${index}`" />
      <RouterLink
        v-for="day in calendar.data.value?.days ?? []"
        :key="day.date"
        :to="drillTo({ from: day.date })"
        class="flex min-w-0 flex-col rounded-sm border px-0.5 py-0.5 hover:ring-1 hover:ring-ring focus-visible:outline-2 focus-visible:outline-ring"
        :class="STATUS_CLASSES[day.status]"
      >
        <span>{{ Number(day.date.slice(8)) }}</span>
        <span class="truncate text-muted-foreground" :class="day.daily === '0' && 'invisible'">
          <Money :amount="day.daily" :currency="calendar.data.value?.currency" :decimals="0" compact :convert="false" />
        </span>
      </RouterLink>
    </div>
  </div>
</template>
