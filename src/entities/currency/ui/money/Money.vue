<script setup lang="ts">
import { computed } from 'vue'

import { isFiatCode, moneyParts, signClass } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { useSettingsStore } from '@/shared/store/settings'

import { moneyMoment } from '../../lib/money-moment'
import { useFavoriteCurrencies } from '../../model/favorites'
import type { MoneyProps } from './money.types'

/**
 * An amount with grouped digits and a separate, muted currency mark.
 *
 * Hovering it shows the amount in the base and favourite currencies, at the
 * rate of `at` when there is one (`MoneyTooltipHost`). A click on it — a tap on
 * a phone — opens the sum card: the amount in the base currency at the
 * operation's rate and today's, and in the favourites (`MoneyCardHost`); inside
 * a button or a link the click is the button's, and a hold opens the card.
 * For the base currency, any fiat, any favourite and any tracked coin. Both
 * hosts are one for the whole app.
 */
const props = withDefaults(defineProps<MoneyProps>(), {
  currency: '',
  decimals: undefined,
  signed: false,
  colored: false,
  compact: false,
  bare: false,
  convert: undefined,
  at: undefined,
  glance: false,
})

const settings = useSettingsStore()
const favorites = useFavoriteCurrencies()

const code = computed(() => (props.currency || settings.baseCurrency).toLowerCase())
const parts = computed(() =>
  moneyParts(props.amount, {
    currency: code.value,
    decimals: props.decimals,
    signed: props.signed || props.colored,
    compact: props.compact,
  }),
)
const tone = computed(() => (props.colored ? signClass(props.amount) : ''))
const numeric = computed(() => Number(props.amount))
const known = computed(
  () =>
    code.value === settings.baseCurrency ||
    isFiatCode(code.value) ||
    favorites.isFavorite(code.value) ||
    settings.cryptoCurrencies.includes(code.value),
)
const convertible = computed(
  () => (props.convert ?? known.value) && Number.isFinite(numeric.value) && numeric.value !== 0,
)
</script>

<template>
  <!-- One element for good: the card and the tooltip are shared (MoneyCardHost, MoneyTooltipHost
       in the shell) and find this sum by its data attributes, so nothing mounts here (rule 17).
       The dashed underline of a sum a click opens lives in app/styles.css: it depends on the parents. -->
  <span
    :class="
      cn(
        'money inline-flex items-baseline gap-[0.25em] whitespace-nowrap tabular-nums',
        tone,
        convertible && 'money-convertible',
        convertible && props.glance && 'money-glance',
      )
    "
    :data-money-convertible="convertible || undefined"
    :data-money-amount="convertible ? numeric : undefined"
    :data-money-currency="convertible ? code : undefined"
    :data-money-at="convertible && props.at ? moneyMoment(props.at) : undefined"
  >
    <span>{{ parts.number }}</span>
    <span v-if="parts.symbol && !props.bare" class="money-symbol text-[0.85em] font-normal text-muted-foreground">{{ parts.symbol }}</span>
  </span>
</template>
