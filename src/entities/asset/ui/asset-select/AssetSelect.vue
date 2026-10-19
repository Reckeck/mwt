<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useFavoriteCurrencies } from '@/entities/currency'
import { Select, SelectItem } from '@/shared/ui/select'
import { assetKey } from '@/shared/lib/asset-ref'

import type { AssetSelectProps } from './asset-select.types'

/**
 * Choosing one of a known set of assets (rule 14): what a wallet holds, an
 * envelope can earmark. For any asset at all use `AssetInput`; for a
 * currency, `CurrencyPicker`. Two assets with one ticker stay apart by key.
 */
const props = defineProps<AssetSelectProps>()

const asset = defineModel<AssetRef | null>({ required: true })

const { t } = useI18n()
const favorites = useFavoriteCurrencies()

/** Favourites first, in the user's order and starred; the rest keep the order given. */
const options = computed(() => {
  const order = favorites.withBase.value
  const rank = (entry: AssetRef) => {
    const index = order.indexOf(entry.symbol.toLowerCase())
    return index < 0 ? order.length : index
  }
  return props.assets
    .map((entry) => ({
      key: assetKey(entry),
      label: entry.name && entry.name !== entry.symbol ? `${entry.symbol} · ${entry.name}` : entry.symbol,
      favorite: favorites.isFavorite(entry.symbol),
      rank: rank(entry),
      asset: entry,
    }))
    .sort((left, right) => left.rank - right.rank)
})
const selected = computed({
  get: () => (asset.value ? assetKey(asset.value) : ''),
  set: (key: string) => {
    asset.value = options.value.find((option) => option.key === key)?.asset ?? null
  },
})
</script>

<template>
  <Select v-model="selected" :invalid="props.invalid" :placeholder="props.placeholder">
    <SelectItem v-for="option in options" :key="option.key" :value="option.key">
      <span v-if="option.favorite" aria-hidden="true" class="text-warning">★</span>
      <span v-if="option.favorite" class="sr-only">{{ t('currency.favoriteMark') }}</span>
      {{ option.label }}
    </SelectItem>
  </Select>
</template>
