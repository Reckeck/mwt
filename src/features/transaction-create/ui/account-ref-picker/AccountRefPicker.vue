<script setup lang="ts">
import { computed } from 'vue'

import { StoreNodeSelect } from '@/entities/store'
import { useStoreTree } from '@/shared/api/queries'

import { quickWalletNodes } from '../../lib/quick-wallets'

/**
 * The wallet of a quick operation: the wallets an operation can be written on (a sub-account,
 * never a group that has live ones), chosen in the bottom sheet of `StoreNodeSelect`.
 */
const props = defineProps<{ invalid?: boolean; placeholder?: string; variant?: 'field' | 'plate' | 'inline'; caption?: string | ((balance: string) => string) }>()
const model = defineModel<string>({ default: '' })
const tree = useStoreTree()
const nodes = computed(() => quickWalletNodes(tree.data.value))
</script>

<template>
  <StoreNodeSelect
    v-model="model"
    :nodes="nodes"
    :invalid="props.invalid"
    :placeholder="props.placeholder"
    :variant="props.variant"
    :caption="props.caption"
    :aria-label="props.placeholder"
  />
</template>
