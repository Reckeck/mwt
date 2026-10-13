<script setup lang="ts">
import { computed } from 'vue'

import { Select } from '@/shared/ui/select'
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

// Native <option> cannot hold markup: the path says where a node sits.
const options = computed(() =>
  (props.flat ? props.nodes : flattenStoreTree(props.nodes)).map((node) => ({
    id: node.store.id,
    label: node.path.join(' › '),
  })),
)
</script>

<template>
  <Select v-model="storeId" :aria-invalid="props.invalid || undefined" :class="props.invalid && 'border-destructive'">
    <option v-if="props.placeholder" value="" disabled>{{ props.placeholder }}</option>
    <option v-if="props.emptyLabel" value="">{{ props.emptyLabel }}</option>
    <option v-for="option in options" :key="option.id" :value="option.id">{{ option.label }}</option>
  </Select>
</template>
