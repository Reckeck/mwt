<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { TxRowMenuHost, useTxRowMenu } from '@/features/transaction-row-menu'
import { TransactionsList } from '@/entities/transaction'
import { UpcomingList } from '@/widgets/upcoming-list'
import { Button, buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney, formatMonthKey, formatWeekday } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { emptyTxFilter } from '@/shared/lib/tx-filter'

import { STATUS_CLASSES } from './spending-calendar.constants'
import CalendarDayBalance from './ui/CalendarDayBalance.vue'
import CalendarDayCell from './ui/CalendarDayCell.vue'
import { openTxDetails } from '@/shared/lib/tx-details'
import { useIsPhone } from '@/shared/lib/phone'

const { t } = useI18n()
/** «⋯» and right-click on a row of the chosen day. */
const rowMenu = useTxRowMenu()
const isPhone = useIsPhone()

const today = localDate(new Date())
const month = ref(today.slice(0, 7))
const calendar = useQuery({
  queryKey: computed(() => ['calendar', month.value]),
  placeholderData: keepPreviousData,
  queryFn: () => call<CalendarMonth>('get_calendar', { month: month.value }),
})

function shift(delta: number) {
  const [year, number] = month.value.split('-').map(Number)
  const next = new Date(year, number - 1 + delta, 1)
  month.value = localDate(next).slice(0, 7)
  selected.value = null
}

const title = computed(() => formatMonthKey(month.value, { month: 'long', year: 'numeric' }))

/** Monday-first weekday names in the active language. */
const weekdays = computed(() =>
  Array.from({ length: 7 }, (_unused, index) =>
    formatWeekday(new Date(2024, 0, 1 + index), 'short'),
  ),
)

/** How many days the shown month has: placeholders hold the grid's height while it loads. */
const monthDays = computed(() => {
  const [year, number] = month.value.split('-').map(Number)
  return new Date(year, number, 0).getDate()
})

/** Blank cells before the 1st, so the 1st sits under its weekday. */
const lead = computed(() => (new Date(`${month.value}-01T12:00`).getDay() + 6) % 7)

const selected = ref<string | null>(null)
// The day is cut in the user's zone by the backend, same as the calendar cells.
const dayQuery = useQuery({
  // no placeholder: another day's operations under this day's title
  queryKey: computed(() => ['transactions', 'day', selected.value]),
  queryFn: () =>
    call<Transaction[]>('list_transactions', { filter: emptyTxFilter({ from: selected.value, to: selected.value }) }),
  enabled: computed(() => selected.value !== null),
})
const dayTxs = computed(() => (selected.value ? (dayQuery.data.value ?? []) : []))

const openDetails = openTxDetails

const currency = computed(() => calendar.data.value?.currency ?? '')
/** What falls due on each day of the month (reminders, payments), by date. */
const upcomingByDay = computed(() => {
  const byDay = new Map<string, UpcomingItem[]>()
  for (const item of calendar.data.value?.upcoming ?? []) byDay.set(item.date, [...(byDay.get(item.date) ?? []), item])
  return byDay
})
const selectedDay = computed(() => calendar.data.value?.days.find((day) => day.date === selected.value) ?? null)
/** The month's rest as of today (or the month's last day already past); forecasts do not count. */
const latest = computed(
  () => [...(calendar.data.value?.days ?? [])].reverse().find((day) => day.left !== null && !day.forecast) ?? null,
)
</script>

<template>
  <!-- One column at any width: the month, then the chosen day, then what is coming. -->
  <div class="grid gap-4">
    <div class="grid gap-4">
      <Card>
        <template #header>
          <div class="flex items-center justify-between gap-2">
            <p class="font-medium capitalize">{{ title }}</p>
            <div class="flex items-center gap-1 pointer-coarse:gap-2">
              <Button variant="ghost" size="icon-sm"  :aria-label="t('calendar.prev')" @click="shift(-1)">
                <ChevronLeft class="size-4" />
              </Button>
              <Button variant="ghost" size="icon-sm"  :aria-label="t('calendar.next')" @click="shift(1)">
                <ChevronRight class="size-4" />
              </Button>
            </div>
          </div>
          <!-- While the month loads its two summary lines keep their place, so the grid below does not drop. -->
          <template v-if="calendar.isPending.value">
            <p class="invisible text-xs" aria-hidden="true">&nbsp;</p>
            <p class="invisible text-xs" aria-hidden="true">&nbsp;</p>
          </template>
          <p v-if="calendar.data.value?.allowance" class="text-xs text-muted-foreground">
            {{ t('calendar.allowance', { amount: formatMoney(calendar.data.value.allowance, { decimals: 0, currency }) }) }}
          </p>
          <!-- The running balance: the month's pool, what is left of it, what each day after may spend. -->
          <p v-if="calendar.data.value?.budget" class="text-xs text-muted-foreground" data-tour="calendar-balance">
            {{ t('calendar.budgetMonth', { amount: formatMoney(calendar.data.value.budget, { decimals: 0, currency }) }) }}
            <template v-if="latest?.left">
              · {{ t('calendar.leftMonth') }}:
              <span :class="Number(latest.left) < 0 ? 'text-destructive' : 'text-foreground'">
                <Money :amount="latest.left" :decimals="0" :currency="currency" />
              </span>
            </template>
            <template v-if="latest?.per_day_left">
              · {{ t('calendar.perDayLeft') }}: <Money :amount="latest.per_day_left" :decimals="0" :currency="currency" />
            </template>
          </p>
        </template>
        <div class="grid grid-cols-7 gap-1 text-center text-xs">
          <div v-for="name in weekdays" :key="name" class="pb-1 text-muted-foreground capitalize">{{ name }}</div>
          <div v-for="index in lead" :key="`lead-${index}`" />
          <CalendarDayCell
            v-for="day in calendar.data.value?.days ?? []"
            :key="day.date"
            :day="day"
            :currency="currency"
            :today="today"
            :selected="selected === day.date"
            :upcoming="upcomingByDay.get(day.date)?.length ?? 0"
            :compact="isPhone"
            @pick="selected = selected === day.date ? null : day.date"
          />
          <div v-for="index in calendar.isPending.value ? monthDays : 0" :key="`wait-${index}`" class="min-h-16 rounded-md border bg-muted/40" />
        </div>
        <div class="mt-3 flex flex-wrap gap-3 text-xs text-muted-foreground">
          <span v-for="status in ['under', 'near', 'over'] as const" :key="status" class="flex items-center gap-1.5">
            <span class="size-3 rounded-sm border" :class="STATUS_CLASSES[status]" />
            {{ t(`calendar.status.${status}`) }}
          </span>
        </div>
      </Card>

    </div>

    <!-- The chosen day under the month, above what is coming. -->
    <div class="grid gap-4">
      <Card v-if="selected">
        <template #header>
          <div class="flex items-center justify-between gap-2">
            <p class="font-medium">{{ t('calendar.dayTitle', { date: formatDate(selected) }) }}</p>
            <RouterLink
              :to="{ name: 'transactions', query: { from: selected, to: selected } }"
              :class="buttonVariants({ variant: 'link', size: 'sm' })"
              class="h-auto px-0"
            >
              {{ t('calendar.openInList') }}
            </RouterLink>
          </div>
        </template>
        <CalendarDayBalance
          v-if="selectedDay"
          class="mb-3"
          :day="selectedDay"
          :currency="currency"
          :upcoming="upcomingByDay.get(selectedDay.date) ?? []"
        />
        <p v-if="!dayTxs.length" class="text-sm text-muted-foreground">{{ t('calendar.dayEmpty') }}</p>
        <div v-else class="-my-3">
          <TransactionsList :transactions="dayTxs" openable :row-actions="rowMenu.actions" @open="openDetails" />
          <TxRowMenuHost :menu="rowMenu" />
        </div>
      </Card>
      <UpcomingList :days="45" :limit="15" />
    </div>

  </div>
</template>
