<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { dateFormat } from '@/shared/lib/format'

import { useConversions } from '../../model/conversions'
import { useConversionsAt } from '../../model/conversions-at'
import { useFavoriteCurrencies } from '../../model/favorites'
import type { MoneyConversionsProps } from './money-conversions.types'

/**
 * An amount in the base and favourite currencies (then the tracked ones) at
 * the latest cached rate; an older rate is labelled with its date. The body of
 * the hover over a price (`Money`) and of anything else that wants it.
 */
const props = withDefaults(defineProps<MoneyConversionsProps>(), { at: '' })
const { t } = useI18n()
const conversions = useConversions(true)
const favorites = useFavoriteCurrencies()
const dated = useConversionsAt(
  () => Math.abs(props.amount),
  () => props.currency.toLowerCase(),
  () => props.at,
)
const rows = computed(() => (props.at ? dated.rows.value : conversions.convert(props.amount, props.currency)))
const loading = computed(() => (props.at ? dated.pending.value : conversions.table.isPending.value))
const day = computed(() =>
  props.at ? dateFormat({ day: 'numeric', month: 'short', year: 'numeric' }).format(new Date(props.at)) : '',
)
</script>

<template>
  <p v-if="loading" class="opacity-70">{{ t('money.loadingRates') }}</p>
  <p v-else-if="!rows.length" class="opacity-70">{{ props.at ? t('money.noRatesOn', { date: day }) : t('money.noRates') }}</p>
  <div v-else class="flex flex-col gap-1">
    <p v-if="props.at" class="opacity-70" data-money-rates-at>{{ t('money.ratesAt', { date: day }) }}</p>
    <dl class="grid grid-cols-[auto_auto] gap-x-3 gap-y-0.5 tabular-nums">
    <template v-for="row in rows" :key="row.currency">
      <dt class="text-right">
        <span v-if="favorites.isFavorite(row.currency)" aria-hidden="true" class="text-warning">★ </span>
        <span v-if="favorites.isFavorite(row.currency)" class="sr-only">{{ t('currency.favoriteMark') }}</span>
        {{ row.text }}
      </dt>
      <dd class="opacity-70">{{ row.stale ? t('money.rateOn', { date: row.stale }) : '' }}</dd>
    </template>
    </dl>
  </div>
</template>
