<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { pageEntityKey } from '@/shared/lib/entity-link'
import { storeIcon } from '@/shared/lib/store-icons'
import { ListRow } from '@/shared/ui/list-row'

import type { StoreRowLinkProps } from './store-row-link.types'

/** An account as a list row that opens its page: icon, name, its kind (and sub-accounts) under it, the sum at the end (the `#value` slot, a `<Money>`). */
const props = defineProps<StoreRowLinkProps>()
defineSlots<{ value?(): unknown }>()
const { t } = useI18n()

const target = computed(() => ({ name: 'wallet', params: { id: props.node.store.id } }))
const subtitle = computed(() => {
  const kind = t(`stores.kind.${props.node.store.kind}`)
  const count = props.node.children.length
  return count ? `${kind} · ${t('stores.subAccounts')}: ${count}` : kind
})
const tone = computed(() => (Number(props.node.subtree_base) < 0 ? 'negative' : 'ink'))
</script>

<template>
  <RouterLink
    :to="target"
    :data-entity="pageEntityKey(target)"
    class="block rounded-md px-1 outline-none focus-visible:ring-2 focus-visible:ring-ring pointer-fine:hover:bg-muted/50"
    data-store-row
  >
    <ListRow :title="props.node.store.name" :subtitle="subtitle" :icon="storeIcon(props.node.store)" :tone="tone">
      <template #value><slot name="value" /></template>
    </ListRow>
  </RouterLink>
</template>
