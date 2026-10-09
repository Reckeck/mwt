<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Plus, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'
import { call, parseError } from '@/shared/api/ipc'
import { CHAIN_LABELS, CHAINS } from '@/shared/lib/chains'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { CONTACT_KINDS, DETAIL_KINDS } from './contact-edit.constants'
import type { ContactDialogProps } from './contact-edit.types'

const props = withDefaults(defineProps<ContactDialogProps>(), { contact: null })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [contact: Contact] }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const name = ref('')
const kind = ref<ContactKind>('person')
const note = ref('')
const accounts = ref<ContactAccount[]>([])

const blankAccount = (): ContactAccount => ({
  id: '',
  group: null,
  label: '',
  kind: 'card',
  value: '',
  currency: null,
  network: null,
  active: true,
})

// Every opening starts from the contact as stored (or blank).
watch(open, (isOpen) => {
  if (!isOpen) return
  name.value = props.contact?.name ?? ''
  kind.value = props.contact?.kind ?? 'person'
  note.value = props.contact?.note ?? ''
  accounts.value = props.contact?.accounts.map((account) => ({ ...account })) ?? []
})

const valid = computed(() => name.value.trim() !== '' && accounts.value.every((account) => account.value.trim()))

const save = useMutation({
  mutationFn: () => {
    const draft: ContactDraft = {
      name: name.value.trim(),
      kind: kind.value,
      note: note.value.trim(),
      accounts: accounts.value.map((account) => ({
        ...account,
        group: account.group?.trim() || null,
        currency: account.currency?.trim().toLowerCase() || null,
        network: account.kind === 'crypto_address' ? account.network : null,
      })),
    }
    return props.contact
      ? call<Contact>('update_contact', { input: { id: props.contact.id, contact: draft } })
      : call<Contact>('create_contact', { input: draft })
  },
  onSuccess: (contact) => {
    toast.success(props.contact ? t('contacts.saved') : t('contacts.created'), { description: contact.name })
    void queryClient.invalidateQueries({ queryKey: ['contacts'] })
    open.value = false
    emit('saved', contact)
  },
  onError: (err) => toast.error(t('contacts.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.contact ? t('contacts.edit') : t('contacts.new')" size="lg">
    <form id="contact-form" class="@container grid gap-4" @submit.prevent="save.mutate()">
      <div class="grid gap-4 @sm:grid-cols-[1fr_12rem]">
        <div class="grid gap-1.5">
          <Label for="contact-name">{{ t('contacts.name') }}</Label>
          <Input id="contact-name" v-model="name" />
        </div>
        <div class="grid gap-1.5">
          <Label for="contact-kind">{{ t('contacts.kind') }}</Label>
          <Select id="contact-kind" v-model="kind">
            <option v-for="entry in CONTACT_KINDS" :key="entry" :value="entry">{{ t(`contactKind.${entry}`) }}</option>
          </Select>
        </div>
      </div>

      <div class="grid gap-2">
        <div class="flex items-center justify-between gap-2">
          <Label>{{ t('contacts.accounts') }}</Label>
          <Button type="button" variant="ghost" size="sm" @click="accounts.push(blankAccount())">
            <Plus class="size-4" />
            {{ t('contacts.addAccount') }}
          </Button>
        </div>
        <p v-if="!accounts.length" class="text-xs text-muted-foreground">{{ t('contacts.accountsHint') }}</p>
        <div
          v-for="(account, index) in accounts"
          :key="account.id || index"
          class="grid gap-2 rounded-md border border-border p-3 @md:grid-cols-[9rem_1fr_1fr_auto]"
          :class="!account.active && 'opacity-60'"
        >
          <Select v-model="account.kind" :aria-label="t('contacts.accountKind')">
            <option v-for="entry in DETAIL_KINDS" :key="entry" :value="entry">{{ t(`detailKind.${entry}`) }}</option>
          </Select>
          <Input v-model="account.value" :placeholder="t('contacts.accountValue')" spellcheck="false" />
          <Input v-model="account.label" :placeholder="t('contacts.accountLabel')" />
          <Button
            type="button"
            variant="ghost"
            size="icon"
            :title="t('contacts.removeAccount')"
            :aria-label="t('contacts.removeAccount')"
            @click="accounts.splice(index, 1)"
          >
            <Trash2 class="size-4" />
          </Button>
          <Input
            :model-value="account.group ?? ''"
            :placeholder="t('contacts.accountGroup')"
            @update:model-value="account.group = String($event) || null"
          />
          <Input
            :model-value="account.currency ?? ''"
            :placeholder="t('contacts.accountCurrency')"
            @update:model-value="account.currency = String($event) || null"
          />
          <Select
            v-if="account.kind === 'crypto_address'"
            :model-value="account.network ?? ''"
            :aria-label="t('form.network')"
            @update:model-value="account.network = ($event as Chain) || null"
          >
            <option value="">{{ t('form.noNetwork') }}</option>
            <option v-for="chain in CHAINS" :key="chain" :value="chain">{{ CHAIN_LABELS[chain] }}</option>
          </Select>
        </div>
        <p v-if="props.contact" class="text-xs text-muted-foreground">{{ t('contacts.removeHint') }}</p>
      </div>

      <div class="grid gap-1.5">
        <Label for="contact-note">{{ t('stores.note') }}</Label>
        <Textarea id="contact-note" v-model="note" rows="2" />
      </div>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="contact-form" type="submit" :disabled="!valid || save.isPending.value">
        {{ t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
