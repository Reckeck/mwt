<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { useRoute, useRouter } from 'vue-router'
import { Plus } from 'lucide-vue-next'

import { ContactEditDialog } from '@/features/contact-edit'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { contactNets, filterContacts, isQuickContact, quickContacts } from '@/entities/contact'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { useContacts, useStoreTree } from '@/shared/api/queries'
import { useIsPhone } from '@/shared/lib/phone'
import { flattenStoreTree } from '@/shared/lib/store-tree'

import type { ContactTransfer } from './model/contact-transfer.types'
import { useContactActions } from './model/use-contact-actions'
import { ContactDetails } from './ui/contact-details'
import { ContactList } from './ui/contact-list'

/**
 * Contacts and debts (Ops-1 · 6): the list on the left, the picked contact on
 * the right. The pick lives in the address (`?contact=`), so on a phone the
 * contact is a screen of its own and `Back` returns to the list.
 */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const phone = useIsPhone()

const contacts = useContacts()
const balances = useQuery({ queryKey: ['contacts', 'balances'], queryFn: () => call<ContactBalance[]>('get_contact_balances') })
const tree = useStoreTree()

const search = ref('')
const quickOnly = ref(false)
const quickCount = computed(() => quickContacts(contacts.data.value ?? []).length)
watch(quickCount, (count) => {
  if (!count) quickOnly.value = false
})
/** Who owes or is owed comes first: that is why the page is opened. Waits for the debts so the order does not jump. */
const list = computed(() => {
  if (balances.isPending.value) return []
  const rows = balances.data.value ?? []
  const owing = new Set(rows.map((row) => row.contact_id).filter((id) => contactNets(rows, id).length))
  const found = filterContacts(contacts.data.value ?? [], search.value)
  return (quickOnly.value ? found.filter((contact) => !contact.archived && isQuickContact(contact)) : found).sort(
    (left, right) => Number(left.archived) - Number(right.archived) || Number(owing.has(right.id)) - Number(owing.has(left.id)),
  )
})

const selectedId = computed({
  get: () => (typeof route.query.contact === 'string' ? route.query.contact : ''),
  set: (id: string) => {
    const to = { query: { ...route.query, contact: id || undefined } }
    // A phone steps into the contact, so `Back` has somewhere to go; a desktop just swaps the right side.
    void (phone.value && id ? router.push(to) : router.replace(to))
  },
})
/** The picked contact; on a desktop the first one shown, so the right side is never blank. */
const selected = computed(
  () =>
    (contacts.data.value ?? []).find((contact) => contact.id === selectedId.value) ??
    (phone.value ? null : (list.value[0] ?? null)),
)

/* --- a transfer or a debt operation with the selected contact --- */
const transferOpen = ref(false)
const transfer = ref<ContactTransfer>({ effect: null, accountId: null })
/** Whom the form is for: a right click acts on a contact before the address catches up. */
const transferContactId = ref('')
/** Operations with a contact start from the first everyday wallet, changeable in the form. */
const transferStore = computed(() => {
  const nodes = flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived)
  return (nodes.find((node) => node.store.purpose === 'everyday') ?? nodes[0])?.store.id ?? ''
})
const transferBlocked = computed(() => {
  if (selected.value?.archived) return t('contacts.transferArchived')
  return transferStore.value ? false : t('contacts.transferNoStore')
})
const counterpart = computed<AccountRef | null>(() =>
  transferContactId.value ? { contact: { contact_id: transferContactId.value, account_id: transfer.value.accountId } } : null,
)
function openTransfer(next: ContactTransfer, contactId = selected.value?.id ?? '') {
  transfer.value = next
  transferContactId.value = contactId
  transferOpen.value = true
}

const contactActions = useContactActions(transferStore, (contact, effect) => {
  selectedId.value = contact.id
  openTransfer({ effect, accountId: null }, contact.id)
})

const createOpen = ref(false)
function back() {
  if (window.history.state?.back) router.back()
  else selectedId.value = ''
}
</script>

<template>
  <Teleport v-if="!phone" defer to="#tx-section-end">
    <Button variant="outline" data-tour="contact-quick-create" @click="createOpen = true">
      <Plus class="size-4" />{{ t('contacts.add') }}
    </Button>
  </Teleport>

  <ContactDetails
    v-if="phone && selected"
    :contact="selected"
    :balances="balances.data.value ?? []"
    :transfer-blocked="transferBlocked"
    phone
    @transfer="openTransfer"
    @deleted="back"
    @back="back"
  />
  <div v-else-if="phone" class="grid gap-3">
    <ContactList
      v-model:search="search"
      v-model:quick-only="quickOnly"
      :quick-count="quickCount"
      :contacts="list"
      :total="contacts.data.value?.length ?? 0"
      :balances="balances.data.value ?? []"
      :loaded="contacts.isSuccess.value"
      :selected-id="null"
      :actions-of="contactActions"
      phone
      @select="(id) => (selectedId = id)"
    />
    <Button variant="outline" class="h-12 w-full" data-tour="contact-quick-create" @click="createOpen = true">
      <Plus class="size-4" />{{ t('contacts.add') }}
    </Button>
  </div>
  <div v-else class="grid items-start gap-4 @container @2xl:grid-cols-[17.5rem_1fr]">
    <ContactList
      v-model:search="search"
      v-model:quick-only="quickOnly"
      :quick-count="quickCount"
      :contacts="list"
      :total="contacts.data.value?.length ?? 0"
      :balances="balances.data.value ?? []"
      :loaded="contacts.isSuccess.value"
      :selected-id="selected?.id ?? null"
      :actions-of="contactActions"
      class="@2xl:min-h-[40rem]"
      @select="(id) => (selectedId = id)"
    />
    <ContactDetails
      v-if="selected"
      :contact="selected"
      :balances="balances.data.value ?? []"
      :transfer-blocked="transferBlocked"
      class="@2xl:min-h-[40rem]"
      @transfer="openTransfer"
      @deleted="selectedId = ''"
    />
    <EmptyState v-else-if="contacts.isSuccess.value" :title="t('contacts.pickOne')" :description="t('contacts.pickOneHint')" />
  </div>

  <ContactEditDialog v-model:open="createOpen" @saved="(contact) => (selectedId = contact.id)" />
  <TransactionCreateDialog
    v-if="transferStore"
    v-model:open="transferOpen"
    :store-id="transferStore"
    :counterpart="counterpart"
    :debt="transfer.effect"
  />
</template>
