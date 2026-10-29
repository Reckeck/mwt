<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { CarryOverSwitch } from '@/features/month-carry-over'
import { Money } from '@/entities/currency'
import { formatDayMonth, formatMonthKey } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { SpendingMonths } from '../spending'
import MonthCarryLine from './MonthCarryLine.vue'
import MonthDaysChart from './MonthDaysChart.vue'

/**
 * `Month's balance` as a phone page (Money 3 · 15): what is left now with the month
 * or the twelve months under it, what it is made of in three rows, and the last
 * months with a link to all twelve.
 */
const props = defineProps<{ balance: MonthBalance; row: MonthBalanceRow; month: string }>()
const emit = defineEmits<{ pick: [month: string] }>()

const { t } = useI18n()
const view = ref<'month' | 'year'>('month')
const showAll = ref(false)
const monthName = computed(() => formatMonthKey(props.month, { month: 'long' }))
const sinceKey = computed(() => (props.balance.carry_since ? `period.since.${props.balance.carry_since.slice(5, 7)}` : null))
const last = computed(() => props.balance.days[props.balance.days.length - 1] ?? null)
const forecast = computed(() =>
  props.row.current && props.balance.per_day && last.value?.forecast
    ? { date: formatDayMonth(`${last.value.date}T12:00`), perDay: props.balance.per_day, end: last.value.forecast }
    : null,
)
const tone = (value: string) => (Number(value) < 0 ? 'text-warning' : 'text-success')
const months = computed(() => {
  const list = [...props.balance.rows].reverse().filter((row) => !row.current)
  return showAll.value ? list : list.slice(0, 2)
})
</script>

<template>
  <div class="grid gap-3" data-month-phone>
    <section class="grid gap-2 rounded-xl border bg-card p-4 shadow-sm">
      <p class="text-sm text-muted-foreground">{{ t('balance.phone.now') }}</p>
      <p class="text-3xl font-bold tabular-nums"><Money :amount="props.row.total" :currency="props.balance.currency" :decimals="0" /></p>
      <i18n-t v-if="forecast" keypath="balance.phone.caption" tag="p" class="text-sm text-muted-foreground">
        <template #date>{{ forecast.date }}</template>
        <template #perDay><Money :amount="forecast.perDay" :currency="props.balance.currency" :decimals="0" /></template>
        <template #end><Money :amount="forecast.end" :currency="props.balance.currency" :decimals="0" /></template>
      </i18n-t>
      <p v-else class="text-sm text-muted-foreground">{{ t('balance.phone.captionNoForecast') }}</p>
      <ToggleGroup v-model="view" type="single" variant="outline" class="w-fit" :aria-label="t('balance.phone.now')">
        <ToggleGroupItem value="month" class="capitalize">{{ monthName }}</ToggleGroupItem>
        <ToggleGroupItem value="year">{{ t('balance.phone.year') }}</ToggleGroupItem>
      </ToggleGroup>
      <MonthDaysChart v-if="view === 'month'" bare :balance="props.balance" :title="monthName" />
      <template v-else>
        <MonthCarryLine v-if="props.balance.carry_since" :balance="props.balance" @pick="emit('pick', $event)" />
        <SpendingMonths v-else bare :balance="props.balance" :from="props.month" :to="props.month" @pick="emit('pick', $event)" />
      </template>
    </section>
    <section class="grid rounded-xl border bg-card px-4 shadow-sm">
      <div class="flex min-h-14 items-center justify-between border-b">
        <span class="text-sm font-medium">{{ t('balance.income') }}</span>
        <b class="text-success"><Money :amount="props.row.income" :currency="props.balance.currency" :decimals="0" signed /></b>
      </div>
      <div class="flex min-h-14 items-center justify-between" :class="sinceKey && 'border-b'">
        <span class="text-sm font-medium">{{ t('balance.spent') }}</span>
        <b class="text-destructive"><Money :amount="`-${props.row.spent}`.replace('--', '')" :currency="props.balance.currency" :decimals="0" /></b>
      </div>
      <div v-if="sinceKey" class="flex min-h-14 items-center justify-between">
        <span class="text-sm font-medium first-letter:uppercase">{{ t(sinceKey) }}</span>
        <b><Money :amount="props.row.carried_in" :currency="props.balance.currency" :decimals="0" signed /></b>
      </div>
    </section>
    <section class="grid gap-1 rounded-xl border bg-card px-4 py-3 shadow-sm">
      <CarryOverSwitch class="min-h-11" :carry-since="props.balance.carry_since" :from="props.balance.rows[0]?.month ?? props.month" />
      <p class="flex justify-between text-xs text-muted-foreground">
        <span>{{ t('balance.phone.sample') }}</span><span>{{ t('balance.phone.perMonth') }}</span>
      </p>
      <ul class="grid">
        <li v-for="item in months" :key="item.month" class="border-b last:border-0">
          <Button variant="ghost" class="h-auto min-h-11 w-full justify-between px-0 py-2 font-normal" @click="emit('pick', item.month)">
            <span class="grid text-left">
              <span class="font-medium capitalize">{{ formatMonthKey(item.month, { month: 'long' }) }}</span>
              <span v-if="props.balance.carry_since" class="text-xs text-muted-foreground">
                <i18n-t keypath="balance.totalNow">
                  <template #total><Money :amount="item.total" :currency="props.balance.currency" :decimals="0" /></template>
                </i18n-t>
              </span>
            </span>
            <b :class="cn('whitespace-nowrap', tone(item.left))"><Money :amount="item.left" :currency="props.balance.currency" :decimals="0" signed /></b>
          </Button>
        </li>
      </ul>
      <Button variant="link" class="h-11 justify-between px-0" @click="showAll = !showAll">
        {{ showAll ? t('balance.phone.fewer') : t('balance.phone.all') }}
        <span aria-hidden="true">{{ showAll ? '‹' : '›' }}</span>
      </Button>
    </section>
  </div>
</template>
