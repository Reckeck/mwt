<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { StoreNodeSelect } from '@/entities/store'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree, storeDescendants } from '@/shared/lib/store-tree'
import { cn } from '@/shared/lib/utils'
import { Checkbox } from '@/shared/ui/checkbox'
import { Label } from '@/shared/ui/label'

import type { FilterFieldProps } from '../../transaction-filter.types'

/** «Кошелёк»: several wallets at once, with or without their sub-accounts. */
const props = defineProps<FilterFieldProps>()
const filter = defineModel<TxFilter>({ required: true })

const { t } = useI18n()
const tree = useStoreTree()
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const storeIds = computed({
  get: () => filter.value.scope.store_ids,
  set: (ids: string[]) => {
    filter.value = { ...filter.value, scope: { store_ids: ids, include_children: filter.value.scope.include_children } }
  },
})
/** Sub-accounts the picked wallets have: the «+N» next to «with sub-accounts». */
const childCount = computed(() =>
  nodes.value
    .filter((entry) => storeIds.value.includes(entry.store.id))
    .reduce((sum, node) => sum + storeDescendants(node).length, 0),
)
const includeChildren = computed({
  get: () => filter.value.scope.include_children,
  set: (value: boolean) => {
    filter.value = { ...filter.value, scope: { ...filter.value.scope, include_children: value } }
  },
})
const id = computed(() => `${props.idPrefix}-scope`)
</script>

<template>
  <div :class="cn('min-w-0', props.bare ? 'contents' : 'grid content-start gap-1.5')">
    <Label v-if="!props.bare" :for="id">{{ t('filter.scope') }}</Label>
    <StoreNodeSelect
      :id="id"
      v-model:ids="storeIds"
      multiple
      :nodes="nodes"
      :empty-label="t('tx.allStores')"
      :aria-label="props.bare ? t('filter.scope') : undefined"
      :class="props.bare ? 'w-full min-w-36 flex-1 basis-36 @3xl:max-w-[200px]' : 'w-full'"
    />
    <label v-if="storeIds.length && childCount" :for="`${id}-children`" class="flex h-9 items-center gap-2 text-sm">
      <Checkbox :id="`${id}-children`" v-model="includeChildren" />
      {{ t('filter.withChildren', { count: childCount }) }}
    </label>
  </div>
</template>
