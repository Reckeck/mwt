<script setup lang="ts">
import { computed } from 'vue'

import { Select } from '@/shared/ui/select'
import { flattenStoreTree } from '@/shared/lib/store-tree'

import type { StoreNodeSelectProps } from './store-node-select.types'

const props = defineProps<StoreNodeSelectProps>()

/** Selected store id; `''` is the empty option. */
const storeId = defineModel<string>({ required: true })

// Native <option> cannot hold markup: the path says where a node sits, so two
// "Spot" sub-accounts of different exchanges stay distinguishable.
const options = computed(() =>
  flattenStoreTree(props.nodes).map((node) => ({ id: node.store.id, label: node.path.join(' › ') })),
)
</script>

<template>
  <Select v-model="storeId">
    <option v-if="props.emptyLabel" value="">{{ props.emptyLabel }}</option>
    <option v-for="option in options" :key="option.id" :value="option.id">{{ option.label }}</option>
  </Select>
</template>
