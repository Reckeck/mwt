<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { LiabilityTermsFields, emptyLiabilityTerms } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'
import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { StoreEditDialogProps } from './store-edit.types'

const props = defineProps<StoreEditDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const name = ref('')
const note = ref('')
const color = ref('')
const inBudget = ref(false)
const terms = ref<LiabilityTerms>(emptyLiabilityTerms())
const contactId = ref('')

/** Only a top-level wallet belongs to a contact (its sub-accounts share it). */
const isRoot = computed(() => !props.store.parent_id)
const isLoan = computed(() => props.store.kind === 'liability')

const contacts = useQuery({
  queryKey: ['contacts'],
  queryFn: () => call<Contact[]>('list_contacts'),
  enabled: isRoot,
})

watch(open, (isOpen) => {
  if (!isOpen) return
  name.value = props.store.name
  note.value = props.store.note
  color.value = props.store.color
  inBudget.value = props.store.in_budget
  terms.value = props.store.liability ? { ...props.store.liability } : emptyLiabilityTerms()
  contactId.value = props.store.contact_id ?? ''
})

const save = useMutation({
  mutationFn: () =>
    call<Store>('update_store', {
      input: {
        id: props.store.id,
        name: name.value.trim(),
        color: color.value,
        note: note.value.trim(),
        in_budget: inBudget.value,
        liability: isLoan.value ? terms.value : null,
        contact_id: isRoot.value ? contactId.value || null : null,
      },
    }),
  onSuccess: () => {
    toast.success(t('stores.saved'))
    for (const queryKey of [['stores'], ['store'], ['summary'], ['net-worth']]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    open.value = false
  },
  onError: (err) => toast.error(t('stores.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="t('stores.edit')" size="sm">
    <form id="store-edit-form" class="@container grid gap-4" @submit.prevent="save.mutate()">
      <div class="grid grid-cols-[1fr_auto] gap-4">
        <div class="grid gap-1.5">
          <Label for="store-edit-name">{{ t('stores.name') }}</Label>
          <Input id="store-edit-name" v-model="name" />
        </div>
        <div class="grid gap-1.5">
          <Label for="store-edit-color">{{ t('stores.color') }}</Label>
          <input
            id="store-edit-color"
            v-model="color"
            type="color"
            class="h-9 w-14 cursor-pointer rounded-md border border-input bg-transparent p-1"
          >
        </div>
      </div>
      <LiabilityTermsFields v-if="isLoan" v-model="terms" />
      <div v-if="isRoot" class="grid gap-1.5">
        <Label for="store-edit-contact">{{ t('stores.contact') }}</Label>
        <Select id="store-edit-contact" v-model="contactId">
          <option value="">{{ t('tx.side.unspecified') }}</option>
          <option v-for="contact in contacts.data.value ?? []" :key="contact.id" :value="contact.id">{{ contact.name }}</option>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t('stores.contactHint') }}</p>
      </div>
      <div class="grid gap-1">
        <div class="flex items-center gap-2">
          <input id="store-edit-in-budget" v-model="inBudget" type="checkbox" class="size-4">
          <Label for="store-edit-in-budget">{{ t('stores.inBudget') }}</Label>
        </div>
        <p class="text-xs text-muted-foreground">{{ t('stores.inBudgetHint') }}</p>
      </div>
      <div class="grid gap-1.5">
        <Label for="store-edit-note">{{ t('stores.note') }}</Label>
        <Textarea id="store-edit-note" v-model="note" rows="2" />
      </div>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="store-edit-form" type="submit" :disabled="!name.trim() || save.isPending.value">
        {{ t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
