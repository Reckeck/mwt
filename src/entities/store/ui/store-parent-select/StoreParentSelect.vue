<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Select } from '@/shared/ui/select'
import { buildStoreTree, flattenStoreTree, storeSubtreeIds } from '@/shared/lib/store-tree'

import type { StoreParentSelectProps } from './store-parent-select.types'

const props = defineProps<StoreParentSelectProps>()

/** Parent wallet id; `''` means top level. */
const parentId = defineModel<string>({ required: true })

const { t } = useI18n()

const options = computed(() => {
  const blocked = props.excludeId ? storeSubtreeIds(props.stores, props.excludeId) : new Set<string>()
  return flattenStoreTree(buildStoreTree(props.stores))
    .filter((node) => !blocked.has(node.store.id))
    .map((node) => ({
      id: node.store.id,
      // Native <option> cannot hold markup; indent with non-breaking spaces.
      label: `${'   '.repeat(node.depth)}${node.store.name}`,
    }))
})
</script>

<template>
  <Select v-model="parentId">
    <option value="">{{ t('stores.parentNone') }}</option>
    <option v-for="option in options" :key="option.id" :value="option.id">{{ option.label }}</option>
  </Select>
</template>
