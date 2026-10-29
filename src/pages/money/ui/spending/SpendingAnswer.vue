<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMonthKey, signClass } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
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
  /** Some spending has no rate: the sum is a floor, `at least` (Currency-1 · 7). */
  incomplete?: boolean
}>()

const { t } = useI18n()
const route = useRoute()
const phone = useIsPhone()
const categories = useCategories()
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
  if (!top.value) return { key: 'money.answer.nothing', category: '' }
  const category = categoryKeyLabel(top.value.key, categories.data.value, t)
  const key = props.rows.length === 1 ? 'money.answer.all' : 'money.answer.most'
  return { key: props.incomplete ? `${key}AtLeast` : key, category }
})

/** `Month in progress: day 22 of 31` while the period is the running month. */
const progress = computed(() => {
  if (!single.value || props.to !== currentMonth()) return null
  const now = new Date()
  const days = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate()
  return t('money.answer.running', { day: now.getDate(), days })
})
const carrySince = computed(() => props.balance.carry_since)
</script>

<template>
  <section class="grid gap-3 rounded-xl border bg-card p-4 shadow-sm" data-tour="spending-answer">
    <h2 class="text-xl font-semibold text-balance first-letter:uppercase"><i18n-t :keypath="headline.key">
        <template #when>{{ when }}</template>
        <template #spent><Money :amount="props.balance.total.spent" :currency="props.balance.currency" :decimals="0" /></template>
        <template #category>{{ headline.category }}</template>
      </i18n-t>
    </h2>
    <p class="flex flex-wrap items-baseline gap-x-1.5 gap-y-1 text-sm">
      <span class="text-muted-foreground">
        <template v-if="progress">{{ progress }}, </template><i18n-t keypath="money.answer.average">
          <template #average><Money :amount="props.average" :currency="props.balance.currency" :decimals="0" /></template>
        </i18n-t>
      </span>
      <template v-if="!phone">
        <span class="text-muted-foreground">{{ t('money.answer.income') }}</span>
        <b class="whitespace-nowrap"><Money :amount="props.balance.total.income" :currency="props.balance.currency" :decimals="0" /></b>
        <span class="text-muted-foreground">· {{ t('money.answer.left') }}</span>
        <b :class="['whitespace-nowrap', signClass(props.balance.total.left)]">
          <Money :amount="props.balance.total.left" :currency="props.balance.currency" :decimals="0" />
        </b>
        <template v-if="carrySince">
          <span class="text-muted-foreground">· {{ t('money.answer.since', { month: monthName(carrySince, 'since') }) }}</span>
          <b class="whitespace-nowrap text-primary">
            <Money glance :amount="props.balance.total.total" :currency="props.balance.currency" :decimals="0" />
          </b>
        </template>
      </template>
      <RouterLink :to="{ name: 'money-month', query: route.query }" class="font-medium whitespace-nowrap text-primary hover:underline">
        {{ t('money.tab.month') }} →
      </RouterLink>
    </p>
    <!-- On a phone the two figures are cards under the phrase (Money 1 · 1). -->
    <div v-if="phone" class="grid grid-cols-2 gap-2" data-spending-cards>
      <div class="rounded-lg border p-3">
        <p class="text-xs text-muted-foreground">{{ t('money.answer.income') }}</p>
        <p class="text-xl font-semibold"><Money :amount="props.balance.total.income" :currency="props.balance.currency" :decimals="0" /></p>
      </div>
      <div class="rounded-lg border p-3">
        <p class="text-xs text-muted-foreground">{{ t('money.answer.leftCard') }}</p>
        <p :class="['text-xl font-semibold', signClass(props.balance.total.left)]">
          <Money :amount="props.balance.total.left" :currency="props.balance.currency" :decimals="0" />
        </p>
        <p v-if="carrySince" class="text-xs text-muted-foreground">
          {{ t('money.answer.since', { month: monthName(carrySince, 'since') }) }}
          <Money glance :amount="props.balance.total.total" :currency="props.balance.currency" :decimals="0" />
        </p>
      </div>
    </div>
  </section>
</template>
