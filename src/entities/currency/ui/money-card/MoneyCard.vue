<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { TIME_CYCLE, currencySymbol, dateFormat, formatMoney, moneyParts } from '@/shared/lib/format'

import { useMoneyCard } from '../../model/money-card'
import type { MoneyCardProps } from './money-card.types'

/**
 * The body of the sum card (Currency-1 · 5): the sum, what it was in the base
 * currency at the operation's rate and what it is today, then the favourites.
 * The popover or the sheet around it, and its keys, are `MoneyCardHost`'s.
 */
const props = defineProps<MoneyCardProps>()
const { t } = useI18n()
const card = useMoneyCard(
  () => props.amount,
  () => props.currency,
  () => props.at,
)

const day = (iso: string) => dateFormat({ day: 'numeric', month: 'short' }).format(new Date(iso))
const moment = (iso: string) =>
  dateFormat({ day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit', hourCycle: TIME_CYCLE }).format(new Date(iso))

const title = computed(() => {
  if (!props.at) return t('moneyCard.amount')
  return t(props.amount < 0 ? 'moneyCard.gave' : 'moneyCard.got', { date: day(props.at) })
})
const size = computed(() => formatMoney(Math.abs(props.amount), { currency: props.currency }))
const money = (value: number | null) => (value === null ? '—' : formatMoney(value, { currency: card.base.value }))
const rate = (value: number | null) =>
  value === null ? '' : t('moneyCard.rate', { price: formatMoney(value, { currency: props.currency }), unit: currencySymbol(card.base.value) })

/** The base-currency line: at the operation's rate when it has a moment, else at today's. */
const main = computed(() => (props.at ? card.atMoment.value : card.now.value))
const mainRate = computed(() => (props.at ? card.atMomentRate.value : card.nowRate.value))
</script>

<template>
  <div class="flex flex-col gap-3 text-sm" data-money-card>
    <div class="flex flex-col gap-0.5">
      <span class="text-xs text-muted-foreground">{{ title }}</span>
      <span class="text-lg font-semibold tabular-nums">{{ size }}</span>
    </div>
    <template v-if="card.foreign.value">
      <div class="flex items-baseline gap-3 rounded-lg bg-muted px-2.5 py-2">
        <span class="min-w-0 flex-1">{{ t(props.at ? 'moneyCard.inBaseThen' : 'moneyCard.inBaseNow') }}</span>
        <b class="tabular-nums" data-money-card-base>{{ card.pending.value && main === null ? '…' : money(main) }}</b>
      </div>
      <div v-if="mainRate !== null" class="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 text-xs text-muted-foreground tabular-nums">
        <span class="min-w-0 flex-1">{{ rate(mainRate) }}<template v-if="props.at"> · {{ moment(props.at) }}</template></span>
        <span v-if="props.at && card.nowRate.value !== null" data-money-card-today>
          {{ t('moneyCard.today', { rate: formatMoney(card.nowRate.value, { currency: props.currency }), value: money(card.now.value) }) }}
        </span>
      </div>
    </template>
    <template v-if="card.favorites.value.length">
      <div class="h-px bg-border" />
      <span class="text-xs text-muted-foreground">{{ props.at ? t('moneyCard.favoritesAt', { date: day(props.at) }) : t('moneyCard.favorites') }}</span>
      <div v-for="row in card.favorites.value" :key="row.currency" class="flex items-baseline gap-3">
        <span class="min-w-0 flex-1">
          <span aria-hidden="true" class="text-warning">★</span>
          <span class="sr-only">{{ t('currency.favoriteMark') }}</span>
          {{ row.currency.toUpperCase() }}
        </span>
        <b class="tabular-nums">{{ moneyParts(row.value, { currency: row.currency }).number }}</b>
      </div>
    </template>
  </div>
</template>
