<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'

import { TransactionEditButton } from '@/features/transaction-edit'
import { TransactionsList } from '@/entities/transaction'
import { TransactionDetailsSheet } from '@/widgets/transaction-details'
import { UpcomingList } from '@/widgets/upcoming-list'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'
import { localDate } from '@/shared/lib/planning'
import { txId } from '@/shared/lib/transactions'
import { emptyTxFilter } from '@/shared/lib/tx-filter'

import { STATUS_CLASSES } from './spending-calendar.constants'

const { t } = useI18n()

const month = ref(localDate(new Date()).slice(0, 7))
const calendar = useQuery({
  queryKey: computed(() => ['calendar', month.value]),
  queryFn: () => call<CalendarMonth>('get_calendar', { month: month.value }),
})

function shift(delta: number) {
  const [year, number] = month.value.split('-').map(Number)
  const next = new Date(year, number - 1 + delta, 1)
  month.value = localDate(next).slice(0, 7)
  selected.value = null
}

const title = computed(() =>
  new Intl.DateTimeFormat(activeLocale.value, { month: 'long', year: 'numeric' }).format(new Date(`${month.value}-01T12:00`)),
)

/** Monday-first weekday names in the active language. */
const weekdays = computed(() =>
  Array.from({ length: 7 }, (_unused, index) =>
    new Intl.DateTimeFormat(activeLocale.value, { weekday: 'short' }).format(new Date(2024, 0, 1 + index)),
  ),
)

/** Blank cells before the 1st, so the 1st sits under its weekday. */
const lead = computed(() => (new Date(`${month.value}-01T12:00`).getDay() + 6) % 7)

const selected = ref<string | null>(null)
// The day is cut in the user's zone by the backend, same as the calendar cells.
const dayQuery = useQuery({
  queryKey: computed(() => ['transactions', 'day', selected.value]),
  queryFn: () =>
    call<Transaction[]>('list_transactions', { filter: emptyTxFilter({ from: selected.value, to: selected.value }) }),
  enabled: computed(() => selected.value !== null),
})
const dayTxs = computed(() => (selected.value ? (dayQuery.data.value ?? []) : []))

const openedId = ref<string | null>(null)
const detailsOpen = ref(false)
const opened = computed(() => dayTxs.value.find((tx) => txId(tx) === openedId.value) ?? null)
function openDetails(tx: Transaction) {
  openedId.value = txId(tx)
  detailsOpen.value = true
}
const money = (value: string) => formatMoney(value, { decimals: 0 })
</script>

<template>
  <div class="grid items-start gap-4 @4xl:grid-cols-[1fr_20rem]">
    <div class="grid gap-4">
      <Card>
        <template #header>
          <div class="flex items-center justify-between gap-2">
            <p class="font-medium capitalize">{{ title }}</p>
            <div class="flex items-center gap-1">
              <Button variant="ghost" size="icon" class="size-8" :aria-label="t('calendar.prev')" @click="shift(-1)">
                <ChevronLeft class="size-4" />
              </Button>
              <Button variant="ghost" size="icon" class="size-8" :aria-label="t('calendar.next')" @click="shift(1)">
                <ChevronRight class="size-4" />
              </Button>
            </div>
          </div>
          <p v-if="calendar.data.value?.allowance" class="text-xs text-muted-foreground">
            {{ t('calendar.allowance', { amount: formatMoney(calendar.data.value.allowance, { decimals: 0, currency: calendar.data.value.currency }) }) }}
          </p>
        </template>
        <div class="grid grid-cols-7 gap-1 text-center text-xs">
          <div v-for="name in weekdays" :key="name" class="pb-1 text-muted-foreground capitalize">{{ name }}</div>
          <div v-for="index in lead" :key="`lead-${index}`" />
          <Button
            variant="outline"
            size="block"
            v-for="day in calendar.data.value?.days ?? []"
            :key="day.date"
            class="h-auto text-xs grid min-h-16 content-start gap-0.5 rounded-md border p-1.5 text-left transition-colors"
            :class="[STATUS_CLASSES[day.status], selected === day.date ? 'ring-2 ring-primary' : '']"
            :aria-pressed="selected === day.date"
            @click="selected = selected === day.date ? null : day.date"
          >
            <span class="font-medium">{{ Number(day.date.slice(8)) }}</span>
            <span v-if="Number(day.daily)" class="tabular-nums">−{{ money(day.daily) }}</span>
            <span v-if="Number(day.income)" class="tabular-nums text-success">+{{ money(day.income) }}</span>
          </Button>
        </div>
        <div class="mt-3 flex flex-wrap gap-3 text-xs text-muted-foreground">
          <span v-for="status in ['under', 'near', 'over'] as const" :key="status" class="flex items-center gap-1.5">
            <span class="size-3 rounded-sm border" :class="STATUS_CLASSES[status]" />
            {{ t(`calendar.status.${status}`) }}
          </span>
        </div>
      </Card>

      <Card v-if="selected">
        <template #header>
          <p class="font-medium">{{ t('calendar.dayTitle', { date: formatDate(selected) }) }}</p>
        </template>
        <p v-if="!dayTxs.length" class="text-sm text-muted-foreground">{{ t('calendar.dayEmpty') }}</p>
        <div v-else class="-my-3">
          <TransactionsList :transactions="dayTxs" openable @open="openDetails">
            <template #actions="{ tx }">
              <TransactionEditButton :tx="tx" />
            </template>
          </TransactionsList>
        </div>
      </Card>
    </div>

    <UpcomingList :days="45" :limit="15" />

    <TransactionDetailsSheet v-model:open="detailsOpen" :tx="opened" />
  </div>
</template>
