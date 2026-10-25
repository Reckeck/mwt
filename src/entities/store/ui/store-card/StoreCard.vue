<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRouter } from 'vue-router'
import { Archive, ArchiveRestore, ExternalLink, Trash2 } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { ActionMenu } from '@/shared/ui/action-menu'
import { Card } from '@/shared/ui/card'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { formatDate } from '@/shared/lib/format'
import { storeDescendants } from '@/shared/lib/store-tree'
import { cn } from '@/shared/lib/utils'
import { storeIcon } from '@/shared/lib/store-icons'

import StoreRowLink from '../store-row-link/StoreRowLink.vue'
import type { StoreCardProps } from './store-card.types'

/**
 * A wallet in the list. The whole card opens it (a stretched link); sub-account
 * rows and the «⋯» menu sit above that link. The page confirms a delete.
 */
const props = defineProps<StoreCardProps>()

const emit = defineEmits<{
  toggleArchive: [store: Store]
  delete: [store: Store]
}>()

const { t } = useI18n()
const router = useRouter()

const store = computed(() => props.node.store)
const descendants = computed(() => storeDescendants(props.node))
const assets = computed(() => props.node.top_assets.map((asset) => asset.symbol.toUpperCase()).join(' · '))

/** The backend refuses to delete a wallet with history or sub-accounts; the menu says why. */
const deleteBlocker = computed(() => {
  if (props.node.children.length) return t('stores.deleteHasChildren')
  if (props.node.own_tx_count > 0) return t('stores.deleteHasTransactions')
  return ''
})

const actions = computed<UiAction[]>(() => [
  {
    id: 'open',
    label: t('stores.open'),
    icon: ExternalLink,
    run: () => void router.push({ name: 'wallet', params: { id: store.value.id } }),
  },
  {
    id: 'archive',
    label: store.value.archived ? t('stores.unarchive') : t('stores.archive'),
    icon: store.value.archived ? ArchiveRestore : Archive,
    run: () => emit('toggleArchive', store.value),
  },
  {
    id: 'delete',
    label: t('stores.delete'),
    icon: Trash2,
    destructive: true,
    separated: true,
    disabled: deleteBlocker.value || false,
    run: () => emit('delete', store.value),
  },
])
</script>

<template>
  <ContextMenu :actions="actions">
    <Card :class="cn('relative transition-colors hover:border-ring/40', store.archived && 'opacity-60')">
      <template #header>
        <div class="flex items-center justify-between gap-2">
          <div class="flex min-w-0 items-center gap-2">
            <span
              class="flex size-6 shrink-0 items-center justify-center rounded-md border"
              :style="{ backgroundColor: `${store.color}1A`, color: store.color, borderColor: `${store.color}33` }"
            >
              <component :is="storeIcon(store)" class="size-3.5" />
            </span>
            <div class="min-w-0">
              <!-- The stretched link: its ::after covers the card, so a click anywhere opens the wallet. -->
              <RouterLink
                :to="{ name: 'wallet', params: { id: store.id } }"
                :data-entity="`store:${store.id}`"
                class="block truncate font-medium after:absolute after:inset-0 after:rounded-[inherit] focus-visible:outline-none focus-visible:after:ring-2 focus-visible:after:ring-ring"
              >
                {{ store.name }}
              </RouterLink>
              <p class="truncate text-xs text-muted-foreground">
                {{ t(`stores.kind.${store.kind}`) }}<template v-if="store.purpose === 'everyday' && store.kind !== 'liability'"> · {{ t('purpose.inBudgetBadge') }}</template>
              </p>
            </div>
          </div>
          <div class="relative z-10 flex shrink-0 items-center gap-1">
            <Money class="text-sm font-medium" :amount="props.node.subtree_base" :currency="props.baseCurrency" />
            <ActionMenu :actions="actions" :label="t('stores.actions', { name: store.name })" />
          </div>
        </div>
      </template>

      <div class="flex flex-col gap-3">
        <p v-if="assets || props.node.last_tx_at" class="flex flex-wrap justify-between gap-x-3 text-xs text-muted-foreground">
          <span v-if="assets" class="truncate">{{ assets }}</span>
          <span>
            {{ props.node.last_tx_at ? t('stores.lastActivity', { date: formatDate(props.node.last_tx_at) }) : t('stores.noActivity') }}
          </span>
        </p>
        <ul v-if="descendants.length" class="relative z-10 flex flex-col">
          <li v-for="child in descendants" :key="child.store.id">
            <StoreRowLink
              :store="child.store"
              :depth="child.depth"
              :amount="child.children.length ? child.subtree_base : child.own_base"
              :currency="props.baseCurrency"
            />
          </li>
        </ul>
        <p v-if="store.note" class="truncate text-sm text-muted-foreground">{{ store.note }}</p>
      </div>
    </Card>
  </ContextMenu>
</template>
