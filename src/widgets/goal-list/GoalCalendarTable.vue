<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'

const { t } = useI18n()

const year = ref(new Date().getFullYear())
const calendar = useQuery({
  queryKey: computed(() => ['goal-calendar', year.value]),
  queryFn: () => call<GoalCalendar>('get_goal_calendar', { year: year.value }),
})

const monthName = (key: string) =>
  new Intl.DateTimeFormat(activeLocale.value, { month: 'short' }).format(new Date(`${key}-01T12:00`))
const isFuture = (key: string) => key > new Date().toISOString().slice(0, 7)
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="font-medium">{{ t('goalCalendar.title') }}</p>
        <div class="flex items-center gap-1">
          <Button variant="ghost" size="icon" class="size-8" :aria-label="t('goalCalendar.prev')" @click="year -= 1">
            <ChevronLeft class="size-4" />
          </Button>
          <span class="w-12 text-center text-sm tabular-nums">{{ year }}</span>
          <Button variant="ghost" size="icon" class="size-8" :aria-label="t('goalCalendar.next')" @click="year += 1">
            <ChevronRight class="size-4" />
          </Button>
        </div>
      </div>
    </template>
    <div class="overflow-x-auto">
      <table class="w-full text-sm tabular-nums">
        <thead class="text-xs text-muted-foreground">
          <tr>
            <th class="py-1 pr-2 text-left font-normal">{{ t('goalCalendar.month') }}</th>
            <th class="px-2 py-1 text-right font-normal">{{ t('goalCalendar.need') }}</th>
            <th class="px-2 py-1 text-right font-normal">{{ t('goalCalendar.saved') }}</th>
            <th class="py-1 pl-2 text-right font-normal">{{ t('goalCalendar.status') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="month in calendar.data.value?.months ?? []" :key="month.month" class="border-t border-border">
            <td class="py-1.5 pr-2 capitalize">{{ monthName(month.month) }}</td>
            <td class="px-2 py-1.5 text-right">{{ formatMoney(month.need, { currency: calendar.data.value?.currency }) }}</td>
            <td class="px-2 py-1.5 text-right">{{ formatMoney(month.saved, { currency: calendar.data.value?.currency }) }}</td>
            <td class="py-1.5 pl-2 text-right text-xs">
              <span v-if="Number(month.need) === 0" class="text-muted-foreground">—</span>
              <span v-else-if="month.done" class="text-emerald-600 dark:text-emerald-400">✓</span>
              <span v-else-if="isFuture(month.month)" class="text-muted-foreground">{{ t('goalCalendar.ahead') }}</span>
              <span v-else class="text-amber-600 dark:text-amber-400">
                {{ t('goalCalendar.short', { amount: formatMoney(month.shortfall, { currency: calendar.data.value?.currency }) }) }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </Card>
</template>
