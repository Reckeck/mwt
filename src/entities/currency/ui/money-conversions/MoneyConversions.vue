<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useConversions } from '../../model/conversions'
import { useFavoriteCurrencies } from '../../model/favorites'
import type { MoneyConversionsProps } from './money-conversions.types'

/**
 * An amount in the base and favourite currencies (then the tracked ones) at
 * the latest cached rate; an older rate is labelled with its date. The body of
 * the hover over a price (`Money`) and of anything else that wants it.
 */
const props = defineProps<MoneyConversionsProps>()
const { t } = useI18n()
const conversions = useConversions(true)
const favorites = useFavoriteCurrencies()
const rows = computed(() => conversions.convert(props.amount, props.currency))
</script>

<template>
  <p v-if="conversions.table.isPending.value" class="opacity-70">{{ t('money.loadingRates') }}</p>
  <p v-else-if="!rows.length" class="opacity-70">{{ t('money.noRates') }}</p>
  <dl v-else class="grid grid-cols-[auto_auto] gap-x-3 gap-y-0.5 tabular-nums">
    <template v-for="row in rows" :key="row.currency">
      <dt class="text-right">
        <span v-if="favorites.isFavorite(row.currency)" aria-hidden="true" class="text-warning">★ </span>
        <span v-if="favorites.isFavorite(row.currency)" class="sr-only">{{ t('currency.favoriteMark') }}</span>
        {{ row.text }}
      </dt>
      <dd class="opacity-70">{{ row.stale ? t('money.rateOn', { date: row.stale }) : '' }}</dd>
    </template>
  </dl>
</template>
