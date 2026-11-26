<script setup lang="ts">
import { computed } from 'vue'

import { assetKey } from '@/shared/lib/asset-ref'
import { Combobox } from '@/shared/ui/combobox'

/** One asset out of a fixed list (the assets a wallet holds); `AssetInput` is the free search. */
const props = defineProps<{ assets: AssetRef[]; id?: string; placeholder?: string }>()
const asset = defineModel<AssetRef | null>({ default: null })

const options = computed(() => props.assets.map((entry) => ({ value: assetKey(entry), label: entry.symbol })))
const key = computed({
  get: () => (asset.value ? assetKey(asset.value) : ''),
  set: (value: string | string[]) => {
    asset.value = props.assets.find((entry) => assetKey(entry) === value) ?? null
  },
})
</script>

<template>
  <Combobox v-model="key" :options="options" :id="props.id" :placeholder="props.placeholder" :empty-label="props.placeholder" />
</template>
