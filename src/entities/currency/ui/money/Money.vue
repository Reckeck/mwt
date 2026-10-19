<script setup lang="ts">
import { computed, ref } from 'vue'

import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'
import { isFiatCode, moneyParts, signClass } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { useSettingsStore } from '@/shared/store/settings'

import { useFavoriteCurrencies } from '../../model/favorites'
import { MoneyConversions } from '../money-conversions'
import type { MoneyProps } from './money.types'

/**
 * An amount with grouped digits and a separate, muted currency mark.
 *
 * Hovering it shows the amount in the base and favourite currencies (then the
 * tracked ones) at the latest cached rate — for the base currency, any fiat
 * and any favourite; a rate older than today is labelled with its date.
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
/** The conversions load on the first hover only. */
const hovered = ref(false)
/**
 * The tooltip mounts on the first hover, open: a page of rows has hundreds of
 * sums, and a tooltip each made the page slow to appear.
 */
const armed = ref(false)
function arm() {
  if (!convertible.value) return
  armed.value = true
  hovered.value = true
}
</script>

<template>
  <Tooltip v-if="convertible && armed" default-open @update:open="(isOpen: boolean) => isOpen && (hovered = true)">
    <TooltipTrigger as-child>
      <span :class="cn('money inline-flex cursor-default items-baseline gap-[0.25em] whitespace-nowrap tabular-nums', tone)">
        <span>{{ parts.number }}</span>
        <span v-if="parts.symbol" class="text-[0.85em] font-normal text-muted-foreground">{{ parts.symbol }}</span>
      </span>
    </TooltipTrigger>
    <TooltipContent side="top" class="min-w-44">
      <MoneyConversions v-if="hovered" :amount="numeric" :currency="code" />
    </TooltipContent>
  </Tooltip>
  <span
    v-else
    :class="cn('money inline-flex items-baseline gap-[0.25em] whitespace-nowrap tabular-nums', tone, convertible && 'cursor-default')"
    @pointerenter="arm"
  >
    <span>{{ parts.number }}</span>
    <span v-if="parts.symbol" class="text-[0.85em] font-normal text-muted-foreground">{{ parts.symbol }}</span>
  </span>
</template>
