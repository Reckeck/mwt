<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { Archive, ArchiveRestore, Trash2 } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { Card } from '@/shared/ui/card'
import { formatMoney } from '@/shared/lib/format'
import { storeDescendants } from '@/shared/lib/store-tree'
import { cn } from '@/shared/lib/utils'
import { getStoreIcon } from '@/shared/lib/store-icons'

import type { StoreCardProps } from './store-card.types'

const props = defineProps<StoreCardProps>()

const emit = defineEmits<{
  toggleArchive: [store: Store]
  delete: [id: string]
}>()

const { t } = useI18n()

const store = computed(() => props.node.store)
const descendants = computed(() => storeDescendants(props.node))
const money = (value: string) => formatMoney(value, { currency: props.baseCurrency, decimals: 0 })

/** The backend refuses to delete a wallet with history or sub-accounts; say why up front. */
const deleteBlocker = computed(() => {
  if (props.node.children.length) return t('stores.deleteHasChildren')
  if (props.node.own_tx_count > 0) return t('stores.deleteHasTransactions')
  return ''
})
</script>

<template>
  <Card :class="cn(store.archived && 'opacity-60')">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <div class="flex min-w-0 items-center gap-2">
          <span
            class="flex size-6 shrink-0 items-center justify-center rounded-md border"
            :style="{ backgroundColor: `${store.color}1A`, color: store.color, borderColor: `${store.color}33` }"
          >
            <component :is="getStoreIcon(store.kind)" class="size-3.5" />
          </span>
          <div class="min-w-0">
            <RouterLink :to="{ name: 'wallet', params: { id: store.id } }" class="block truncate font-medium hover:underline">
              {{ store.name }}
            </RouterLink>
            <p class="truncate text-xs text-muted-foreground">{{ t(`stores.kind.${store.kind}`) }}</p>
          </div>
        </div>
        <span class="shrink-0 text-sm font-medium tabular-nums">{{ money(props.node.subtree_base) }}</span>
      </div>
    </template>

    <div class="flex flex-col gap-3">
      <ul v-if="descendants.length" class="flex flex-col">
        <li v-for="child in descendants" :key="child.store.id">
          <RouterLink
            :to="{ name: 'wallet', params: { id: child.store.id } }"
            class="-mx-2 flex items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors hover:bg-accent"
            :class="cn(child.depth > 1 && 'pl-7', child.store.archived && 'opacity-60')"
          >
            <component :is="getStoreIcon(child.store.kind)" class="size-4 shrink-0" :style="{ color: child.store.color }" />
            <span class="min-w-0 flex-1 truncate">{{ child.store.name }}</span>
            <Badge v-if="child.store.archived" variant="outline">{{ t('stores.archived') }}</Badge>
            <span class="shrink-0 tabular-nums text-muted-foreground">
              {{ money(child.children.length ? child.subtree_base : child.own_base) }}
            </span>
          </RouterLink>
        </li>
      </ul>

      <div class="flex items-center justify-between gap-2">
        <p class="min-w-0 truncate text-sm text-muted-foreground">{{ store.note }}</p>
        <div class="flex shrink-0 items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            class="size-8 text-muted-foreground"
            :title="store.archived ? t('stores.unarchive') : t('stores.archive')"
            :aria-label="store.archived ? t('stores.unarchive') : t('stores.archive')"
            @click="emit('toggleArchive', store)"
          >
            <ArchiveRestore v-if="store.archived" class="size-4" />
            <Archive v-else class="size-4" />
          </Button>
          <span :title="deleteBlocker || undefined">
            <ConfirmButton
              :label="t('stores.delete')"
              :confirm-label="t('actions.confirmDelete')"
              :disabled="!!deleteBlocker"
              @confirm="emit('delete', store.id)"
            >
              <Trash2 class="size-4" />
            </ConfirmButton>
          </span>
        </div>
      </div>
    </div>
  </Card>
</template>
