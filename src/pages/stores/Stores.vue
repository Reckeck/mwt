<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { StoreCreateDialog } from '@/features/store-create'
import { StoreCard } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call, parseError } from '@/shared/api/ipc'
import { buildStoreTree, flattenStoreTree } from '@/shared/lib/store-tree'

const { t } = useI18n()
const queryClient = useQueryClient()

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const createDialogOpen = ref(false)

/** Tree order: every parent is followed by its sub-wallets. */
const rows = computed(() => {
  const list = stores.data.value ?? []
  const byId = new Map(list.map((store) => [store.id, store]))
  return flattenStoreTree(buildStoreTree(list)).map((node) => ({
    store: node.store,
    depth: node.depth,
    parent: node.store.parent_id ? (byId.get(node.store.parent_id) ?? null) : null,
  }))
})

const hasChildren = (id: string) => (stores.data.value ?? []).some((store) => store.parent_id === id)

function invalidate() {
  return queryClient.invalidateQueries({ queryKey: ['stores'] })
}

const toggleArchive = useMutation({
  mutationFn: (store: Store) => call('archive_store', { id: store.id, archived: !store.archived }),
  onSuccess: (_result, store) => {
    toast.success(store.archived ? t('stores.unarchivedToast') : t('stores.archivedToast'), { description: store.name })
    void invalidate()
  },
  onError: (err) => toast.error(t('stores.archiveFailed'), { description: parseError(err).message }),
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_store', { id }),
  onSuccess: () => {
    toast.success(t('stores.deletedToast'))
    void invalidate()
  },
  // The backend refuses a wallet that still has history; say what to do instead.
  onError: (err) => {
    const parsed = parseError(err)
    const description = parsed.code === 'INVALID_INPUT' ? t('stores.deleteHasTransactions') : parsed.message
    toast.error(t('stores.deleteFailed'), { description })
  },
})

function removeStore(id: string) {
  if (hasChildren(id)) {
    toast.error(t('stores.deleteFailed'), { description: t('stores.deleteHasChildren') })
    return
  }
  remove.mutate(id)
}
</script>

<template>
  <div class="grid gap-6 @container">
    <div class="flex items-center justify-between gap-4">
      <p class="text-sm text-muted-foreground">{{ t('stores.title') }}</p>
      <Button @click="createDialogOpen = true">
        <Plus class="size-4" />
        {{ t('stores.create') }}
      </Button>
    </div>

    <div class="grid gap-4 @sm:grid-cols-2 @lg:grid-cols-3">
      <StoreCard
        v-for="row in rows"
        :key="row.store.id"
        :store="row.store"
        :parent="row.parent"
        :class="row.depth > 0 && 'border-l-4'"
        :style="row.parent ? { borderLeftColor: row.parent.color } : undefined"
        @toggle-archive="toggleArchive.mutate"
        @delete="removeStore"
      />

      <Card v-if="!stores.data.value?.length">
        <p class="text-sm text-muted-foreground">{{ t('stores.empty') }}</p>
      </Card>
    </div>

    <StoreCreateDialog v-model:open="createDialogOpen" />
  </div>
</template>