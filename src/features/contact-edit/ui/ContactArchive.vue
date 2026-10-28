<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Archive, ArchiveRestore, Trash2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call } from '@/shared/api/ipc'

import type { ContactArchiveProps } from '../contact-edit.types'

const props = defineProps<ContactArchiveProps>()
const emit = defineEmits<{ deleted: [] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const toggle = useMutation({
  mutationFn: () => call('archive_contact', { id: props.contact.id, archived: !props.contact.archived }),
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['contacts'] })
  },
  meta: { error: 'contacts.saveFailed', success: () => props.contact.archived ? t('contacts.unarchived') : t('contacts.archived') },
})

// The backend refuses a contact that transactions refer to; the toast says to archive it instead.
const remove = useMutation({
  mutationFn: () => call('delete_contact', { id: props.contact.id }),
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['contacts'] })
    emit('deleted')
  },
  meta: { error: 'contacts.deleteFailed', success: 'contacts.deleted' },
})
</script>

<template>
  <Button
    variant="ghost"
    size="icon-sm"
    class="text-muted-foreground"
    :title="props.contact.archived ? t('stores.unarchive') : t('stores.archive')"
    :aria-label="props.contact.archived ? t('stores.unarchive') : t('stores.archive')"
    :disabled="toggle.isPending.value"
    @click="toggle.mutate()"
  >
    <component :is="props.contact.archived ? ArchiveRestore : Archive" class="size-4" />
  </Button>
  <ConfirmButton :label="t('contacts.delete')" :confirm-label="t('actions.confirmDeleteNamed', { name: props.contact.name })" @confirm="remove.mutate()">
    <Trash2 class="size-4" />
  </ConfirmButton>
</template>
