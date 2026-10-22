<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { formatDay } from '@/shared/lib/format'
import { Card } from '@/shared/ui/card'

import { num } from './chart'
import { transactionsOfDay } from './saldo'

/**
 * Where the balance comes from: the pool part by part, everyday spending, and
 * the days that took the most — each opens its transactions, so a rent or a
 * one-off bought as «everyday» can be given a mandatory category.
 */
const props = defineProps<{ month: CalendarMonth; spent: string; latest: CalendarDay | null }>()

const { t } = useI18n()

const negativePool = computed(() => num(props.month.budget) < 0)
const rate = computed(() => {
  const pool = props.month.pool
  return pool && num(pool.income) > 0 ? Math.round((num(pool.savings) / num(pool.income)) * 100) : 0
})
const left = computed(() => String(num(props.month.budget) - num(props.spent)))
/** The three days with the most everyday spending. */
const biggest = computed(() =>
  props.month.days
    .filter((day) => !day.forecast && num(day.daily) > 0)
    .sort((left, right) => num(right.daily) - num(left.daily))
    .slice(0, 3),
)
const minus = (value: string) => String(-num(value))
/** Days after the last real one: what the rest is split over. */
const daysAhead = computed(() =>
  props.latest ? props.month.days.length - props.month.days.indexOf(props.latest) - 1 : props.month.days.length,
)
</script>

<template>
  <Card data-tour="saldo-pool">
    <template #header>
      <p class="font-medium">{{ t('saldo.pool.title') }}</p>
    </template>
    <div class="grid gap-4 @wide:grid-cols-2">
      <dl class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-1.5 text-sm tabular-nums">
        <template v-if="props.month.pool">
          <dt class="text-muted-foreground">{{ t('saldo.pool.income') }}</dt>
          <dd class="text-right"><Money :amount="props.month.pool.income" :currency="props.month.currency" :decimals="0" /></dd>
          <dt class="text-muted-foreground">{{ t('saldo.pool.fixed') }}</dt>
          <dd class="text-right">
            <Money :amount="minus(props.month.pool.planned_fixed)" :currency="props.month.currency" :decimals="0" />
          </dd>
          <dt class="text-muted-foreground">{{ t('saldo.pool.savings', { rate }) }}</dt>
          <dd class="text-right">
            <Money :amount="minus(props.month.pool.savings)" :currency="props.month.currency" :decimals="0" />
          </dd>
        </template>
        <dt v-else class="text-muted-foreground">{{ t('saldo.pool.byLimit') }}</dt>
        <dd v-if="!props.month.pool" />
        <dt class="border-t pt-1.5 font-medium">
          {{ t('saldo.pool.total') }}
          <span class="block text-xs font-normal text-muted-foreground">{{ t('saldo.formula.reserve') }}</span>
        </dt>
        <dd class="border-t pt-1.5 text-right font-medium" :class="negativePool && 'text-destructive'">
          <Money :amount="props.month.budget ?? '0'" :currency="props.month.currency" :decimals="0" />
        </dd>
        <dt class="text-muted-foreground">{{ t('saldo.pool.spent') }}</dt>
        <dd class="text-right"><Money :amount="minus(props.spent)" :currency="props.month.currency" :decimals="0" /></dd>
        <dt class="border-t pt-1.5 font-medium">
          {{ t('saldo.pool.left') }}
          <span class="block text-xs font-normal text-muted-foreground">{{ t('saldo.formula.left') }}</span>
        </dt>
        <dd class="border-t pt-1.5 text-right font-medium" :class="num(left) < 0 && 'text-destructive'">
          <Money :amount="left" :currency="props.month.currency" :decimals="0" />
        </dd>
        <dt class="text-muted-foreground">{{ t('saldo.daysAhead') }}</dt>
        <dd class="text-right">{{ daysAhead }}</dd>
        <template v-if="props.latest?.per_day_left">
          <dt class="font-medium">
            {{ t('saldo.perDay') }}
            <span class="block text-xs font-normal text-muted-foreground">{{ t('saldo.formula.perDay') }}</span>
          </dt>
          <dd class="text-right font-medium">
            <Money :amount="props.latest.per_day_left" :currency="props.month.currency" :decimals="0" />
          </dd>
        </template>
      </dl>
      <div class="grid content-start gap-2 text-sm">
        <p v-if="negativePool" class="text-muted-foreground">
          {{ t('saldo.pool.negative') }}
          <RouterLink :to="{ name: 'automations' }" class="text-foreground underline underline-offset-4">
            {{ t('saldo.pool.automations') }}
          </RouterLink>
        </p>
        <template v-if="biggest.length">
          <p class="font-medium">{{ t('saldo.pool.biggest') }}</p>
          <ul class="grid gap-1">
            <li v-for="day in biggest" :key="day.date">
              <RouterLink
                :to="transactionsOfDay(day.date)"
                class="flex items-center justify-between gap-3 rounded-md px-2 py-1.5 tabular-nums hover:bg-muted"
              >
                <span class="capitalize">{{ formatDay(`${day.date}T12:00`) }}</span>
                <Money :amount="day.daily" :currency="props.month.currency" :decimals="0" />
              </RouterLink>
            </li>
          </ul>
          <p class="text-xs text-muted-foreground">{{ t('saldo.pool.biggestHint') }}</p>
        </template>
      </div>
    </div>
  </Card>
</template>
