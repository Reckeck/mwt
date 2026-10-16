<script setup lang="ts">
import { computed } from 'vue'

import { Select, SelectItem } from '@/shared/ui/select'
import { flattenStoreTree } from '@/shared/lib/store-tree'

import type { StoreNodeSelectProps } from './store-node-select.types'

/**
 * The one way to choose a wallet or a sub-account (rule 14): every node shows
 * as its full path, so two «Spot» sub-accounts of different exchanges stay
 * apart, and the list is the same wherever an account is picked.
 */
const props = defineProps<StoreNodeSelectProps>()

/** Selected store id; `''` is the empty option. */
const storeId = defineModel<string>({ required: true })

// Native <SelectItem> cannot hold markup: the path says where a node sits.
const options = computed(() =>
  (props.flat ? props.nodes : flattenStoreTree(props.nodes)).map((node) => ({
    id: node.store.id,
    label: node.path.join(' › '),
  })),
)
</script>

<template>
  <Select v-model="storeId" :invalid="props.invalid" :placeholder="props.placeholder">
    <SelectItem v-if="props.emptyLabel" value="">{{ props.emptyLabel }}</SelectItem>
    <SelectItem v-for="option in options" :key="option.id" :value="option.id">{{ option.label }}</SelectItem>
  </Select>
</template>
