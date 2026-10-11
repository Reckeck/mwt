<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

const emit = defineEmits<{ created: [contact: Contact] }>()
const { t } = useI18n()
const queryClient = useQueryClient()

const name = ref('')

/** A contact starts as just a name; everything else is filled in its card. */
const create = useMutation({
  mutationFn: () =>
    call<Contact>('create_contact', { input: { name: name.value.trim(), kind: 'person', note: '', accounts: [], group: '' } }),
  onSuccess: (contact) => {
    name.value = ''
    void queryClient.invalidateQueries({ queryKey: ['contacts'] })
    emit('created', contact)
  },
  onError: (err) => toast.error(t('contacts.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <form class="flex items-center gap-1" data-tour="contact-quick-create" @submit.prevent="name.trim() && create.mutate()">
    <Input v-model="name" class="h-9" :placeholder="t('contacts.quickCreate')" :aria-label="t('contacts.quickCreate')" />
    <Button
      type="submit"
      size="icon"
      class="size-9 shrink-0"
      :disabled="!name.trim() || create.isPending.value"
      :title="t('contacts.new')"
      :aria-label="t('contacts.new')"
    >
      <Plus class="size-4" />
    </Button>
  </form>
</template>
