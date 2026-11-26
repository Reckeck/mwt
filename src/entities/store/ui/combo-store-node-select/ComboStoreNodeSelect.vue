<script setup lang="ts">
import { computed } from 'vue'

import { Combobox } from '@/shared/ui/combobox'

/**
 * Wallets and sub-accounts out of the store tree (a combobox; the sheet picker of the quick add is a different one), each by its full path so two «Spot» accounts stay
 * apart: one pick, or `multiple` for chips. The caller passes the nodes it allows.
 */
const props = defineProps<{
  nodes: StoreTreeNode[]
  multiple?: boolean
  id?: string
  placeholder?: string
  emptyLabel?: string
  disabled?: boolean
}>()
const model = defineModel<string | string[]>({ required: true })

const options = computed(() => props.nodes.map((node) => ({ value: node.store.id, label: node.path.join(' › ') })))
</script>

<template>
  <Combobox
    v-model="model"
    :options="options"
    :multiple="props.multiple"
    :id="props.id"
    :placeholder="props.placeholder"
    :empty-label="props.emptyLabel ?? props.placeholder"
    :disabled="props.disabled"
  />
</template>
