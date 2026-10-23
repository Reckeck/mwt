<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { TxRowMenuHost, useTxRowMenu } from '@/features/transaction-row-menu'
import { TransactionsList, dayLabel } from '@/entities/transaction'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatMonthKey, formatWeekday } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { emptyTxFilter } from '@/shared/lib/tx-filter'
import { openTxDetails } from '@/shared/lib/tx-details'
import { useIsPhone } from '@/shared/lib/phone'

import CalendarDayCell from './ui/CalendarDayCell.vue'
import CalendarLegend from './ui/CalendarLegend.vue'
import CalendarMonthHeader from './ui/CalendarMonthHeader.vue'

/**
 * The list's second view: the month by days. On a computer the month's
 * header sits on the tabs' row (`headerTo`); on a phone the grid shows dots
 * and the chosen day's operations below it, with the list's filters.
 */
const props = defineProps<{ filter?: TxFilter; headerTo?: string }>()
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
/** Today's «можно», the same number as on Home. */
const daily = useQuery({ queryKey: ['daily-budget'], queryFn: () => call<DailyBudget>('get_daily_budget') })

const selected = ref<string | null>(isPhone.value ? today : null)
watch(isPhone, (phone) => {
  if (phone && !selected.value) selected.value = today
})

function shift(delta: number) {
  const [year, number] = month.value.split('-').map(Number)
  month.value = localDate(new Date(year, number - 1 + delta, 1)).slice(0, 7)
  selected.value = isPhone.value && month.value === today.slice(0, 7) ? today : null
}

// «октябрь 2026 г.» → «октябрь 2026», as on the canvas.
const title = computed(() => formatMonthKey(month.value, { month: 'long', year: 'numeric' }).replace(/\s*г\.$/, ''))

/** Monday-first weekday names in the active language. */
const weekdays = computed(() =>
  Array.from({ length: 7 }, (_unused, index) => formatWeekday(new Date(2024, 0, 1 + index), 'short')),
)

/** The neighbouring months' days that fill the first and last weeks, dimmed. */
const edges = computed(() => {
  const [year, number] = month.value.split('-').map(Number)
  const lead = (new Date(year, number - 1, 1).getDay() + 6) % 7
  const length = new Date(year, number, 0).getDate()
  const before = Array.from({ length: lead }, (_unused, index) => new Date(year, number - 1, index - lead + 1).getDate())
  const after = Array.from({ length: (7 - ((lead + length) % 7)) % 7 }, (_unused, index) => index + 1)
  return { before, after, length }
})

// The chosen day with the list's filters, cut in the user's zone by the backend.
const dayQuery = useQuery({
  // no placeholder: another day's operations under this day's title
  queryKey: computed(() => ['transactions', 'day', selected.value, props.filter]),
  queryFn: () =>
    call<Transaction[]>('list_transactions', {
      filter: { ...(props.filter ?? emptyTxFilter()), from: selected.value, to: selected.value },
    }),
  enabled: computed(() => selected.value !== null),
})
const dayTxs = computed(() => (selected.value ? (dayQuery.data.value ?? []) : []))

const currency = computed(() => calendar.data.value?.currency ?? '')
const days = computed(() => calendar.data.value?.days ?? [])
const selectedDay = computed(() => days.value.find((day) => day.date === selected.value) ?? null)
/** The month's rest as of today (or the month's last day already past); forecasts do not count. */
const latest = computed(() => [...days.value].reverse().find((day) => day.left !== null && !day.forecast) ?? null)
const leftToday = computed(() => (month.value === today.slice(0, 7) ? (daily.data.value?.left_today ?? null) : null))
const totals = computed(() => {
  const past = days.value.filter((day) => !day.forecast)
  return {
    income: past.reduce((sum, day) => sum + Number(day.income), 0),
    spent: past.reduce((sum, day) => sum + Number(day.spent), 0),
    over: past.filter((day) => day.status === 'over').length,
  }
})
</script>

<template>
  <div class="grid gap-3">
    <Teleport v-if="props.headerTo && !isPhone" defer :to="props.headerTo">
      <CalendarMonthHeader
        :title="title"
        :currency="currency"
        :left="latest?.left ?? null"
        :per-day="latest?.per_day_left ?? null"
        :compact="false"
        @shift="shift"
      />
    </Teleport>
    <CalendarMonthHeader
      v-else
      :title="title"
      :currency="currency"
      :left="latest?.left ?? null"
      :per-day="latest?.per_day_left ?? null"
      :compact="isPhone"
      @shift="shift"
    />

    <div class="grid grid-cols-7 gap-px overflow-hidden rounded-[10px] border bg-border" data-tour="calendar-grid">
      <div v-for="name in weekdays" :key="name" class="bg-muted px-2 py-1 text-xs text-muted-foreground capitalize">
        {{ name }}
      </div>
      <div
        v-for="number in edges.before"
        :key="`before-${number}`"
        class="bg-card px-2 py-1.5 text-sm text-muted-foreground opacity-50"
        :class="isPhone ? 'min-h-11 text-center' : 'min-h-[92px]'"
      >
        {{ number }}
      </div>
      <template v-if="days.length">
        <CalendarDayCell
          v-for="day in days"
          :key="day.date"
          :day="day"
          :currency="currency"
          :today="today"
          :selected="selected === day.date"
          :left-today="leftToday"
          :compact="isPhone"
          @pick="selected = selected === day.date && !isPhone ? null : day.date"
        />
      </template>
      <template v-else>
        <div v-for="index in edges.length" :key="`wait-${index}`" class="bg-card" :class="isPhone ? 'min-h-11' : 'min-h-[92px]'" />
      </template>
      <div
        v-for="number in edges.after"
        :key="`after-${number}`"
        class="bg-card px-2 py-1.5 text-sm text-muted-foreground opacity-50"
        :class="isPhone ? 'min-h-11 text-center' : 'min-h-[92px]'"
      >
        {{ number }}
      </div>
    </div>

    <CalendarLegend
      v-if="!isPhone && calendar.data.value"
      :currency="currency"
      :income="totals.income"
      :spent="totals.spent"
      :per-day="latest?.per_day_left ?? null"
      :over-days="totals.over"
    />

    <!-- The chosen day: its sum and its operations, with the list's filters. -->
    <section v-if="selected" class="grid gap-2">
      <p class="text-sm text-muted-foreground">
        {{ dayLabel(selected) }}
        <template v-if="selectedDay && Number(selectedDay.spent) > 0">
          · −<Money :amount="selectedDay.spent" :currency="currency" />
        </template>
      </p>
      <Card class="py-0">
        <p v-if="!dayTxs.length" class="py-4 text-sm text-muted-foreground">{{ t('calendar.dayEmpty') }}</p>
        <template v-else>
          <TransactionsList :transactions="dayTxs" openable :row-actions="rowMenu.actions" @open="openTxDetails" />
          <TxRowMenuHost :menu="rowMenu" />
        </template>
      </Card>
    </section>
  </div>
</template>
