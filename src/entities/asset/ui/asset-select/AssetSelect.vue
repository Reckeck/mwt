<script setup lang="ts">
import { computed } from 'vue'

import { Select } from '@/shared/ui/select'
import { assetKey } from '@/shared/lib/asset-ref'

import type { AssetSelectProps } from './asset-select.types'

/**
 * Choosing one of a known set of assets (rule 14): what a wallet holds, an
 * envelope can earmark. For any asset at all use `AssetInput`; for a
 * currency, `CurrencyPicker`. Two assets with one ticker stay apart by key.
 */
const props = defineProps<AssetSelectProps>()

const asset = defineModel<AssetRef | null>({ required: true })

const options = computed(() =>
  props.assets.map((entry) => ({
    key: assetKey(entry),
    label: entry.name && entry.name !== entry.symbol ? `${entry.symbol} · ${entry.name}` : entry.symbol,
    asset: entry,
  })),
)
const selected = computed({
  get: () => (asset.value ? assetKey(asset.value) : ''),
  set: (key: string) => {
    asset.value = options.value.find((option) => option.key === key)?.asset ?? null
  },
})
</script>

<template>
  <Select v-model="selected" :aria-invalid="props.invalid || undefined" :class="props.invalid && 'border-destructive'">
    <option v-if="props.placeholder" value="" disabled>{{ props.placeholder }}</option>
    <option v-for="option in options" :key="option.key" :value="option.key">{{ option.label }}</option>
  </Select>
</template>
