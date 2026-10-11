<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { StoreCreateDialog } from '@/features/store-create'
import { StoreCard } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { call, parseError } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const router = useRouter()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const tree = useQuery({
  queryKey: STORE_TREE_QUERY_KEY,
  queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
})

const roots = computed(() => tree.data.value ?? [])
const createDialogOpen = ref(false)

function invalidate() {
  for (const queryKey of [['stores'], ['store'], ['summary']]) void queryClient.invalidateQueries({ queryKey })
}

const toggleArchive = useMutation({
  mutationFn: (store: Store) => call('archive_store', { id: store.id, archived: !store.archived }),
  onSuccess: (_result, store) => {
    toast.success(store.archived ? t('stores.unarchivedToast') : t('stores.archivedToast'), { description: store.name })
    invalidate()
  },
  onError: (err) => toast.error(t('stores.archiveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_store', { id }),
  onSuccess: () => {
    toast.success(t('stores.deletedToast'))
    invalidate()
  },
  onError: (err) => toast.error(t('stores.deleteFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

function openWallet(created: Store) {
  void router.push({ name: 'wallet', params: { id: created.id } })
}
</script>

<template>
  <div class="grid gap-4 @container">
    <div class="flex items-center justify-between gap-4">
      <p class="text-sm text-muted-foreground">{{ t('stores.title') }}</p>
      <Button data-tour="stores-create" @click="createDialogOpen = true">
        <Plus class="size-4" />
        {{ t('stores.create') }}
      </Button>
    </div>

    <div class="grid items-start gap-4 @sm:grid-cols-2 @lg:grid-cols-3" data-tour="stores-list">
      <StoreCard
        v-for="node in roots"
        :key="node.store.id"
        :node="node"
        :base-currency="settings.baseCurrency"
        @toggle-archive="toggleArchive.mutate"
        @delete="remove.mutate"
      />
    </div>

    <div
      v-if="tree.isSuccess.value && !roots.length"
      class="flex flex-col items-center gap-2 rounded-xl border border-dashed border-border p-8 text-center"
    >
      <p class="text-sm text-muted-foreground">{{ t('stores.empty') }}</p>
      <Button class="mt-2" @click="createDialogOpen = true">
        <Plus class="size-4" />
        {{ t('stores.create') }}
      </Button>
    </div>

    <StoreCreateDialog v-model:open="createDialogOpen" @created="openWallet" />
  </div>
</template>
