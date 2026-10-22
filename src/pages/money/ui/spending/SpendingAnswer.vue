<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney, formatMonthKey, signClass } from '@/shared/lib/format'
import { currentMonth } from '@/shared/lib/period'

import type { SpendingCategory } from '../../model/use-spending'

/**
 * The answer above everything (Money 1 · 1): how much went, and on what; then
 * where the month stands — what came in, what is left, what has piled up.
 */
const props = defineProps<{
  from: string
  to: string
  balance: MonthBalance
  /** The average month of the year that ends with the period. */
  average: string
  rows: SpendingCategory[]
}>()

const { t } = useI18n()
const route = useRoute()
const categories = useCategories()
const money = (value: string) => formatMoney(value, { decimals: 0, currency: props.balance.currency })
const monthName = (key: string, form: 'in' | 'since') => t(`period.${form}.${key.slice(5, 7)}`)

const single = computed(() => props.from === props.to)
const top = computed(() => props.rows[0] ?? null)
const when = computed(() =>
  single.value
    ? monthName(props.from, 'in')
    : t('money.answer.between', {
        from: formatMonthKey(props.from, { month: 'short', year: 'numeric' }),
        to: formatMonthKey(props.to, { month: 'short', year: 'numeric' }),
      }),
)
const headline = computed(() => {
  const spent = money(props.balance.total.spent)
  if (!top.value) return t('money.answer.nothing', { when: when.value })
  const category = categoryKeyLabel(top.value.key, categories.data.value, t)
  const key = props.rows.length === 1 ? 'money.answer.all' : 'money.answer.most'
  return t(key, { when: when.value, spent, category })
})

/** «Месяц идёт: день 22 из 31» while the period is the running month. */
const progress = computed(() => {
  if (!single.value || props.to !== currentMonth()) return null
  const now = new Date()
  const days = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate()
  return t('money.answer.running', { day: now.getDate(), days })
})
const carrySince = computed(() => props.balance.carry_since)
</script>

<template>
  <section class="grid gap-2.5 rounded-xl border bg-card p-4 shadow-sm" data-tour="spending-answer">
    <h2 class="text-xl font-semibold text-balance first-letter:uppercase">{{ headline }}</h2>
    <p class="flex flex-wrap items-baseline gap-x-1.5 gap-y-1 text-sm">
      <span class="text-muted-foreground">
        <template v-if="progress">{{ progress }}, </template>{{ t('money.answer.average', { average: money(props.average) }) }}
      </span>
      <span class="text-muted-foreground">{{ t('money.answer.income') }}</span>
      <b class="whitespace-nowrap">{{ money(props.balance.total.income) }}</b>
      <span class="text-muted-foreground">· {{ t('money.answer.left') }}</span>
      <b :class="['whitespace-nowrap', signClass(props.balance.total.left)]">{{ money(props.balance.total.left) }}</b>
      <template v-if="carrySince">
        <span class="text-muted-foreground">· {{ t('money.answer.since', { month: monthName(carrySince, 'since') }) }}</span>
        <b class="whitespace-nowrap border-b border-dashed border-current text-primary">{{ money(props.balance.total.total) }}</b>
      </template>
      <RouterLink :to="{ name: 'money-month', query: route.query }" class="font-medium whitespace-nowrap text-primary hover:underline">
        {{ t('money.tab.month') }} →
      </RouterLink>
    </p>
  </section>
</template>
