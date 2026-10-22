<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { comparisonDelta } from '@/shared/lib/comparison'
import { formatMoney, formatPct, signClass } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

import type { useHomeToday } from '../../model/use-home-today'

/**
 * The three figures on top of Home (Home 1, 5): the capital, this month's spending
 * and what can be spent today. On a phone the day's allowance comes first, filled.
 * A fresh vault shows grey zeros with a hint of where each figure will come from.
 */
const props = defineProps<{ home: ReturnType<typeof useHomeToday> }>()
const { t } = useI18n()

const today = computed(() => props.home.today.data.value)
const daily = computed(() => props.home.daily.data.value)
const spent = computed(() => props.home.balance.data.value?.total.spent ?? '0')
const spentCurrency = computed(() => props.home.balance.data.value?.currency ?? today.value?.currency ?? '')
const change = computed(() => comparisonDelta(today.value?.net_worth).pct)
/** Nothing to spread yet: no income, fact or plan, this month. */
const noIncome = computed(() => !daily.value || (daily.value.mode === 'month' && Number(daily.value.income) === 0))
/** The phone fills the card with the accent only for good news: red on blue does not read. */
const filled = computed(() => !noIncome.value && Number(daily.value?.left_today) >= 0)

const money = (value: string, currency: string) => formatMoney(value, { decimals: 0, currency })
const dayLine = computed(() => {
  const budget = daily.value
  if (!budget) return ''
  const perDay = money(budget.per_day, budget.currency)
  const carry = Number(budget.carry)
  if (budget.mode !== 'month' || Math.round(carry) === 0) return t('home.figures.perDay', { perDay })
  const key = carry > 0 ? 'home.figures.carryPlus' : 'home.figures.carryMinus'
  return t(key, { perDay, carry: money(String(Math.abs(carry)), budget.currency) })
})
const limitLine = computed(() => {
  const status = props.home.limit.value
  if (!status) return null
  const over = Number(status.remaining) < 0
  return t(over ? 'home.figures.limitOver' : 'home.figures.limitLeft', {
    limit: money(status.budget.limit, status.budget.currency),
    left: money(String(Math.abs(Number(status.remaining))), status.budget.currency),
  })
})
</script>

<template>
  <Card v-if="props.home.error.value" data-tour="home-figures">
    <SectionState compact :error="props.home.error.value" @retry="props.home.refetch()" />
  </Card>
  <div v-else class="grid grid-cols-2 gap-3 md:grid-cols-3 md:gap-4" data-tour="home-figures">
    <Card>
      <div class="grid gap-1">
      <p class="text-xs text-muted-foreground md:text-sm">{{ t('home.figures.capital') }}</p>
      <p :class="cn('value', props.home.fresh.value && 'text-muted-foreground')">
        <Money
          :amount="props.home.summary.data.value?.net_worth_base ?? '0'"
          :currency="props.home.summary.data.value?.base_currency"
          :convert="false"
        />
      </p>
      <p v-if="props.home.fresh.value" class="text-xs text-muted-foreground md:text-sm">{{ t('home.figures.capitalEmpty') }}</p>
      <p v-else-if="change !== null" :class="cn('text-xs tabular-nums md:text-sm', signClass(change))">
        {{ t('home.figures.capitalChange', { pct: formatPct(change, { signed: true, decimals: 1 }) }) }}
      </p>
      </div>
    </Card>

    <Card>
      <div class="grid gap-1">
      <p class="truncate text-xs text-muted-foreground md:text-sm">
        {{ t('home.figures.spent', { when: t(`period.in.${props.home.month.slice(5, 7)}`) }) }}
      </p>
      <p :class="cn('value', props.home.fresh.value && 'text-muted-foreground')">
        <Money :amount="spent" :currency="spentCurrency" :decimals="0" :convert="false" />
      </p>
      <p v-if="props.home.fresh.value" class="text-xs text-muted-foreground md:text-sm">{{ t('home.figures.spentEmpty') }}</p>
      <p v-else-if="limitLine" class="text-xs text-muted-foreground tabular-nums md:text-sm">{{ limitLine }}</p>
      <p v-else class="text-xs md:text-sm">
        <span class="text-muted-foreground max-md:hidden">{{ t('home.figures.noLimit') }} · </span>
        <RouterLink :to="{ name: 'money-plan', query: { new: 'limit' } }" class="text-primary hover:underline">
          <span class="max-md:hidden">{{ t('home.figures.setLimit') }}</span>
          <span class="md:hidden">{{ t('home.figures.setLimitLong') }}</span>
        </RouterLink>
      </p>
      </div>
    </Card>

    <Card
      :class="
        cn(
          'order-first col-span-2 md:order-none md:col-span-1',
          filled && 'max-md:border-0 max-md:bg-primary max-md:text-primary-foreground max-md:[&_.money>span+span]:text-primary-foreground/80',
        )
      "
    >
      <div class="grid gap-1">
      <p class="text-sm text-muted-foreground" :class="filled && 'max-md:text-primary-foreground/85'">
        {{ t('home.figures.leftToday') }}
      </p>
      <p v-if="noIncome" class="value big text-muted-foreground">—</p>
      <p v-else :class="cn('value big', Number(daily?.left_today) < 0 && 'text-destructive')">
        <Money :amount="daily?.left_today ?? '0'" :currency="daily?.currency" :decimals="0" :convert="false" />
      </p>
      <p v-if="noIncome" class="text-xs text-muted-foreground md:text-sm">
        <span class="max-md:hidden">{{ t('home.figures.needIncome') }}</span>
        <span class="md:hidden">{{ t('home.figures.needIncomeShort') }}</span>
      </p>
      <p v-else class="text-sm text-muted-foreground tabular-nums max-md:text-primary-foreground/85">{{ dayLine }}</p>
      </div>
    </Card>
  </div>
</template>

<style scoped>
/* The canvas figure: 28px on a computer, 20px in the phone's pair; the currency mark smaller and muted. */
.value {
  font-size: 20px;
  font-weight: 600;
  letter-spacing: -0.01em;
  line-height: 1.15;
  font-variant-numeric: tabular-nums;
}
.value.big,
.value:where(.big) {
  font-size: 28px;
}
@media (min-width: 768px) {
  .value {
    font-size: 28px;
  }
}
.value :deep(.money > span + span) {
  font-size: 0.7em;
}
.bg-primary .value :deep(.money > span + span) {
  color: inherit;
}
</style>
