<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Archive, ArchiveRestore, Shuffle, Trash2 } from 'lucide-vue-next'

import { StoreChangeDialog } from '@/features/store-change'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call } from '@/shared/api/ipc'

import { STORE_STALE_KEYS } from '../model/use-store-update'
import type { StoreDangerZoneProps } from '../store-edit.types'

const props = defineProps<StoreDangerZoneProps>()
const { t } = useI18n()
const router = useRouter()
const queryClient = useQueryClient()

const isCash = computed(() => props.store.kind === 'cash' && !props.store.parent_id)
const changeOpen = ref(false)

/** Why delete is unavailable: the backend refuses a wallet with history or sub-accounts. */
const deleteBlocker = computed(() => {
  if (isCash.value) return t('errors.CASH_LOCKED')
  if (props.childCount) return t('stores.deleteHasChildren')
  if (props.txCount > 0) return t('stores.deleteHasTransactions')
  return ''
})

const refresh = () => {
  for (const queryKey of STORE_STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
}

const toggleArchive = useMutation({
  mutationFn: (archived: boolean) => call('archive_store', { id: props.store.id, archived }),
  onSuccess: refresh,
  meta: {
    error: 'wallet.archiveFailed',
    success: (_data, archived) => (archived ? t('wallet.archived') : t('wallet.unarchived')),
  },
})

const destroy = useMutation({
  mutationFn: () => call('delete_store', { id: props.store.id }),
  onSuccess: () => {
    refresh()
    void router.push({ name: 'dashboard' })
  },
  meta: { error: 'wallet.deleteFailed', success: 'wallet.deleted' },
})
</script>

<template>
  <div class="grid gap-4">
    <div class="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border p-4" data-tour="store-change">
      <div class="grid gap-0.5">
        <p class="text-sm font-medium">{{ t('storeChange.open') }}</p>
        <p class="text-xs text-muted-foreground">{{ t('storeChange.openHint') }}</p>
      </div>
      <Button variant="outline" :disabled="isCash" @click="changeOpen = true">
        <Shuffle class="size-4" />
        {{ t('storeChange.open') }}
      </Button>
    </div>
    <StoreChangeDialog v-model:open="changeOpen" :store="props.store" />

    <div class="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border p-4">
      <div class="grid gap-0.5">
        <p class="text-sm font-medium">{{ props.store.archived ? t('stores.unarchive') : t('stores.archive') }}</p>
        <p class="text-xs text-muted-foreground">
          {{ isCash ? t('errors.CASH_LOCKED') : props.childCount ? t('stores.archiveCascade') : t('wallet.archivedHint') }}
        </p>
      </div>
      <Button
        variant="outline"
        :disabled="isCash || toggleArchive.isPending.value"
        @click="toggleArchive.mutate(!props.store.archived)"
      >
        <component :is="props.store.archived ? ArchiveRestore : Archive" class="size-4" />
        {{ props.store.archived ? t('stores.unarchive') : t('stores.archive') }}
      </Button>
    </div>

    <div class="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-destructive/40 p-4">
      <div class="grid gap-0.5">
        <p class="text-sm font-medium text-destructive">{{ t('stores.delete') }}</p>
        <p class="text-xs text-muted-foreground">{{ deleteBlocker || t('stores.deleteHint') }}</p>
      </div>
      <ConfirmButton
        :label="t('stores.delete')"
        :confirm-label="t('actions.confirmDelete')"
        :disabled="!!deleteBlocker || destroy.isPending.value"
        @confirm="destroy.mutate()"
      >
        <Trash2 class="size-4" />
      </ConfirmButton>
    </div>
  </div>
</template>
