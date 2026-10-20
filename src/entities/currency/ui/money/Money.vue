<script setup lang="ts">
import { computed } from 'vue'

import { isFiatCode, moneyParts, signClass } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { useSettingsStore } from '@/shared/store/settings'

import { useFavoriteCurrencies } from '../../model/favorites'
import type { MoneyProps } from './money.types'

/**
 * An amount with grouped digits and a separate, muted currency mark.
 *
 * Hovering it shows the amount in the base and favourite currencies (then the
 * tracked ones) at the latest cached rate — for the base currency, any fiat
 * and any favourite; a rate older than today is labelled with its date. The
 * tooltip itself is one for the whole app: `MoneyTooltipHost`.
 */
const props = withDefaults(defineProps<MoneyProps>(), {
  currency: '',
  decimals: undefined,
  signed: false,
  colored: false,
  compact: false,
  convert: undefined,
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
    favorites.isFavorite(code.value),
)
const convertible = computed(
  () => (props.convert ?? known.value) && Number.isFinite(numeric.value) && numeric.value !== 0,
)
</script>

<template>
  <!-- One element for good: the conversions tooltip is shared (MoneyTooltipHost in the shell)
       and finds this sum by its data attributes, so hovering mounts nothing here (rule 17). -->
  <span
    :class="cn('money inline-flex items-baseline gap-[0.25em] whitespace-nowrap tabular-nums', tone, convertible && 'cursor-default')"
    :data-money-convertible="convertible || undefined"
    :data-money-amount="convertible ? numeric : undefined"
    :data-money-currency="convertible ? code : undefined"
  >
    <span>{{ parts.number }}</span>
    <span v-if="parts.symbol" class="text-[0.85em] font-normal text-muted-foreground">{{ parts.symbol }}</span>
  </span>
</template>
