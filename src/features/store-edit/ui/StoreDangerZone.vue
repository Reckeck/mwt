<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Archive, ArchiveRestore, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { STORE_STALE_KEYS } from '../model/use-store-update'
import type { StoreDangerZoneProps } from '../store-edit.types'

const props = defineProps<StoreDangerZoneProps>()
const { t } = useI18n()
const router = useRouter()
const queryClient = useQueryClient()

const isCash = computed(() => props.store.kind === 'cash' && !props.store.parent_id)

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
  onSuccess: (_result, archived) => {
    toast.success(archived ? t('wallet.archived') : t('wallet.unarchived'))
    refresh()
  },
  onError: (err) => toast.error(t('wallet.archiveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const destroy = useMutation({
  mutationFn: () => call('delete_store', { id: props.store.id }),
  onSuccess: () => {
    toast.success(t('wallet.deleted'))
    refresh()
    void router.push({ name: 'dashboard' })
  },
  onError: (err) => toast.error(t('wallet.deleteFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <div class="grid gap-4">
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
