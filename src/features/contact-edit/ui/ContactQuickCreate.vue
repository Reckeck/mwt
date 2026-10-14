<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Plus } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { call } from '@/shared/api/ipc'
import { required, useFormValidation } from '@/shared/lib/form'

const emit = defineEmits<{ created: [contact: Contact] }>()
const { t } = useI18n()
const queryClient = useQueryClient()

const name = ref('')
const form = useFormValidation(() => ({ name: required(name.value) }))

/** A contact starts as just a name; everything else is filled in its card. */
const create = useMutation({
  mutationFn: () =>
    call<Contact>('create_contact', { input: { name: name.value.trim(), kind: 'person', note: '', accounts: [], group: '' } }),
  onSuccess: (contact) => {
    name.value = ''
    form.reset()
    void queryClient.invalidateQueries({ queryKey: ['contacts'] })
    emit('created', contact)
  },
  meta: { success: 'contacts.created', error: 'contacts.saveFailed' },
})
</script>

<template>
  <form class="grid gap-1" data-tour="contact-quick-create" @submit.prevent="form.submit(create.mutate, $event)">
    <div class="flex items-center gap-1" :data-invalid="form.invalidAttr('name')">
      <Input v-model="name" class="h-9" :placeholder="t('contacts.quickCreate')" :aria-label="t('contacts.quickCreate')" />
      <Button
        type="submit"
        size="icon"
        class="size-9 shrink-0"
        :disabled="create.isPending.value"
        :title="t('contacts.new')"
        :aria-label="t('contacts.new')"
      >
        <Plus class="size-4" />
      </Button>
    </div>
    <p v-if="form.firstMessage()" class="text-xs text-destructive" role="alert">{{ form.firstMessage() }}</p>
  </form>
</template>
