<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { StoreCreateDialog } from '@/features/store-create'
import { StoreCard } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call, parseError } from '@/shared/api/ipc'

const { t } = useI18n()
const queryClient = useQueryClient()

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const createDialogOpen = ref(false)

function invalidate() {
  return queryClient.invalidateQueries({ queryKey: ['stores'] })
}

const toggleArchive = useMutation({
  mutationFn: (store: Store) => call('archive_store', { id: store.id, archived: !store.archived }),
  onSuccess: (_result, store) => {
    toast.success(store.archived ? t('stores.unarchived') : t('stores.archived'), { description: store.name })
    void invalidate()
  },
  onError: (err) => toast.error(t('stores.archiveFailed'), { description: parseError(err).message }),
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_store', { id }),
  onSuccess: () => {
    toast.success(t('stores.deleted'))
    void invalidate()
  },
  onError: (err) => toast.error(t('stores.deleteFailed'), { description: parseError(err).message }),
})
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
        v-for="store in stores.data.value ?? []"
        :key="store.id"
        :store="store"
        @toggle-archive="toggleArchive.mutate"
        @delete="remove.mutate"
      />

      <Card v-if="!stores.data.value?.length">
        <p class="text-sm text-muted-foreground">{{ t('stores.empty') }}</p>
      </Card>
    </div>

    <StoreCreateDialog v-model:open="createDialogOpen" />
  </div>
</template>