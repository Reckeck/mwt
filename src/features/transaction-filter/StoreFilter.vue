<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { StoreNodeSelect } from '@/entities/store'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'

/** Selected store id (its sub-accounts included); an empty string means every store. */
const storeId = defineModel<string>({ required: true })

const { t } = useI18n()

const tree = useQuery({
  queryKey: STORE_TREE_QUERY_KEY,
  queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
})
</script>

<template>
  <StoreNodeSelect v-model="storeId" class="h-9 w-56" :nodes="tree.data.value ?? []" :empty-label="t('tx.allStores')" />
</template>
