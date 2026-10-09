<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Archive, ArchiveRestore, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { ContactArchiveProps } from './contact-edit.types'

const props = defineProps<ContactArchiveProps>()
const emit = defineEmits<{ deleted: [] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const toggle = useMutation({
  mutationFn: () => call('archive_contact', { id: props.contact.id, archived: !props.contact.archived }),
  onSuccess: () => {
    toast.success(props.contact.archived ? t('contacts.unarchived') : t('contacts.archived'))
    void queryClient.invalidateQueries({ queryKey: ['contacts'] })
  },
  onError: (err) => toast.error(t('contacts.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

// The backend refuses a contact that transactions refer to; the toast says to archive it instead.
const remove = useMutation({
  mutationFn: () => call('delete_contact', { id: props.contact.id }),
  onSuccess: () => {
    toast.success(t('contacts.deleted'))
    void queryClient.invalidateQueries({ queryKey: ['contacts'] })
    emit('deleted')
  },
  onError: (err) => toast.error(t('contacts.deleteFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Button
    variant="ghost"
    size="icon"
    class="size-8 text-muted-foreground"
    :title="props.contact.archived ? t('stores.unarchive') : t('stores.archive')"
    :aria-label="props.contact.archived ? t('stores.unarchive') : t('stores.archive')"
    :disabled="toggle.isPending.value"
    @click="toggle.mutate()"
  >
    <component :is="props.contact.archived ? ArchiveRestore : Archive" class="size-4" />
  </Button>
  <ConfirmButton :label="t('contacts.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="remove.mutate()">
    <Trash2 class="size-4" />
  </ConfirmButton>
</template>
