<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEventListener } from '@vueuse/core'
import { Plus } from 'lucide-vue-next'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { accountCounts, maskDetail } from '@/entities/contact'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { copyText } from '@/shared/lib/clipboard'
import { pluralForm } from '@/shared/lib/format'
import { useCoarsePointer } from '@/shared/lib/phone'

import type { ContactAccountsProps } from '../contact-edit.types'
import { draftOf } from '../model/use-contact-save'
import ContactAccountForm from './ContactAccountForm.vue'
import ContactAccountRow from './ContactAccountRow.vue'

/**
 * `Requisites` of a contact (Ops-2 · 7–9): records, not inputs. A paste
 * anywhere outside a field (Ctrl V) or into the dashed box opens the form with
 * what the app recognised; each change saves with `Undo` in the toast.
 */
const props = defineProps<ContactAccountsProps>()
const emit = defineEmits<{ transfer: [account: ContactAccount] }>()

const { t, locale } = useI18n()
const plural = (key: string, count: number) => t(`${key}.${pluralForm(count, locale.value)}`, { count })
const queryClient = useQueryClient()
const coarse = useCoarsePointer()

const counts = computed(() => accountCounts(props.contact.accounts))
const groups = computed(() => [
  ...new Set(props.contact.accounts.map((account) => account.group?.trim() ?? '').filter(Boolean)),
])
/** Active first; the inactive ones stay for old operations, below. */
const ordered = computed(() => [...props.contact.accounts].sort((left, right) => Number(right.active) - Number(left.active)))

/** `null` — closed; `''` — a new requisite; an id — that one is being changed. */
const editing = ref<string | null>(null)
const pasted = ref('')
const editedAccount = computed(() => props.contact.accounts.find((account) => account.id === editing.value) ?? null)

function open(value = '') {
  pasted.value = value
  editing.value = ''
}

/** Each change carries the requisites as they were, so its `Undo` puts exactly those back. */
interface AccountsChange {
  accounts: ContactAccount[]
  before: ContactDraft
}
const save = useMutation({
  mutationFn: ({ accounts }: AccountsChange) =>
    call<Contact>('update_contact', { input: { id: props.contact.id, contact: draftOf(props.contact, { accounts }) } }),
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['contacts'] })
    editing.value = null
  },
  meta: {
    success: (_data, vars) => {
      const { accounts, before } = vars as AccountsChange
      return t(accounts.length > before.accounts.length ? 'contacts.accountAdded' : 'contacts.accountSaved')
    },
    error: 'contacts.saveFailed',
    undo: (data, vars) =>
      call('update_contact', { input: { id: (data as Contact).id, contact: (vars as AccountsChange).before } }),
  },
})

function change(accounts: ContactAccount[]) {
  save.mutate({ accounts, before: draftOf(props.contact) })
}

function submit(account: ContactAccount) {
  const rest = props.contact.accounts
  change(account.id ? rest.map((entry) => (entry.id === account.id ? account : entry)) : [...rest, account])
}

function toggle(account: ContactAccount) {
  change(props.contact.accounts.map((entry) => (entry.id === account.id ? { ...entry, active: !entry.active } : entry)))
}

function asPhone(account: ContactAccount, phone: string) {
  change(
    props.contact.accounts.map((entry) =>
      entry.id === account.id ? { ...entry, kind: 'phone', value: phone, network: null } : entry,
    ),
  )
}

function copy(account: ContactAccount) {
  const label = account.label || t(`detailKind.${account.kind}`)
  void copyText(account.value, t('contacts.copiedAccount', { label, masked: maskDetail(account.kind, account.value) }))
}

/** Ctrl V outside any field drops the clipboard into a new requisite. */
useEventListener(document, 'paste', (event: ClipboardEvent) => {
  const target = event.target as HTMLElement | null
  if (editing.value !== null || target?.closest('input, textarea, [contenteditable], [role="dialog"]')) return
  const text = event.clipboardData?.getData('text')?.trim()
  if (!text) return
  event.preventDefault()
  open(text)
})

</script>

<template>
  <section class="@container grid min-w-0 grid-cols-1 gap-3" data-tour="contact-accounts">
    <div class="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
      <div class="flex min-w-0 flex-wrap items-baseline gap-x-3">
        <h3 class="text-sm font-semibold text-muted-foreground">{{ t('contacts.accountsMain') }}</h3>
        <span v-if="props.contact.accounts.length" class="text-xs text-muted-foreground">
          {{ plural('contacts.activeCount', counts.active)
          }}<template v-if="counts.inactive"> · {{ plural('contacts.inactiveCount', counts.inactive) }}</template>
        </span>
      </div>
      <!-- Always there, disabled while the form is open: the heading row never changes. -->
      <Button
        variant="outline"
        :disabled="editing !== null"
        data-tour="contact-add-account"
        data-contact-paste
        @click="open()"
      >
        <Plus class="size-4" />{{ t('contacts.addRequisite') }}
      </Button>
    </div>

    <!-- The new requisite is the first row of the list. -->
    <ContactAccountForm
      v-if="editing === ''"
      :key="pasted || 'new'"
      :initial="null"
      :value="pasted"
      :groups="groups"
      :pending="save.isPending.value"
      @submit="submit"
      @cancel="editing = null"
    />

    <EmptyState
      v-if="!props.contact.accounts.length && editing === null"
      compact
      :title="t('contacts.quick.noAccounts')"
      data-contact-no-accounts
    >
      <Button data-contact-add-accounts @click="open()"><Plus class="size-4" />{{ t('contacts.quick.addAccounts') }}</Button>
    </EmptyState>

    <div v-if="ordered.length" class="divide-y rounded-lg border bg-card px-3">
      <template v-for="account in ordered" :key="account.id">
        <ContactAccountForm
          v-if="editing === account.id"
          class="my-3"
          :initial="editedAccount"
          :value="''"
          :groups="groups"
          :pending="save.isPending.value"
          @submit="submit"
          @cancel="editing = null"
        />
        <ContactAccountRow
          v-else
          :account="account"
          :transfer-disabled="props.transferDisabled"
          @copy="copy(account)"
          @transfer="emit('transfer', account)"
          @edit="editing = account.id"
          @toggle="toggle(account)"
          @as-phone="asPhone(account, $event)"
        />
      </template>
    </div>
    <p v-if="counts.inactive && !coarse" class="text-xs text-muted-foreground">{{ t('contacts.inactiveHint') }}</p>
    <p v-if="coarse && ordered.length" class="text-xs text-muted-foreground">{{ t('contacts.tapHint') }}</p>
  </section>
</template>
