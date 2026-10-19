<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { useFavoriteCurrencies } from '../../model/favorites'

import type { ChartCurrencySwitchProps } from './chart-currency-switch.types'

/** The currency a money chart is drawn in: the base or a favourite. */
const props = defineProps<ChartCurrencySwitchProps>()
const currency = defineModel<string>({ required: true })
const { t } = useI18n()
const favorites = useFavoriteCurrencies()
/** The screen reader hears which codes are favourites; the order (source, base, favourites) shows it on screen. */
const label = (code: string) =>
  favorites.isFavorite(code) ? `${code.toUpperCase()}, ${t('currency.favoriteMark')}` : code.toUpperCase()
</script>

<template>
  <ToggleGroup
    v-if="props.options.length > 1"
    v-model="currency"
    type="single"
    variant="outline"
    size="sm"
    :aria-label="t('charts.currency')"
    data-tour="chart-currency"
  >
    <ToggleGroupItem
      v-for="code in props.options"
      :key="code"
      :value="code"
      :aria-label="label(code)"
      class="px-2 text-xs uppercase"
    >
      {{ code }}
    </ToggleGroupItem>
  </ToggleGroup>
</template>
