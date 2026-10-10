<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { STATUS_CLASSES } from '@/widgets/spending-calendar'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { activeLocale } from '@/shared/lib/locale'
import { localDate } from '@/shared/lib/planning'

const { t } = useI18n()

const month = localDate(new Date()).slice(0, 7)
const calendar = useQuery({
  queryKey: ['calendar', month],
  queryFn: () => call<CalendarMonth>('get_calendar', { month }),
})
const lead = (new Date(`${month}-01T12:00`).getDay() + 6) % 7
const title = new Intl.DateTimeFormat(activeLocale.value, { month: 'long' }).format(new Date(`${month}-01T12:00`))
const upcomingDates = computed(() => new Set((calendar.data.value?.upcoming ?? []).map((item) => item.date)))
</script>

<template>
  <Card v-if="calendar.data.value">
    <template #header>
      <RouterLink :to="{ name: 'transactions-calendar' }" class="text-sm text-muted-foreground capitalize hover:text-foreground">
        {{ t('miniCalendar.title', { month: title }) }}
      </RouterLink>
    </template>
    <div class="grid grid-cols-7 gap-1 text-center text-[11px] tabular-nums">
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
</template>
