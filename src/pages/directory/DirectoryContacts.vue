<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { TransactionCreateDialog } from '@/features/transaction-create'
import { type ContactDebtFilter, contactSections, filterContacts } from '@/entities/contact'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'

import { useContactActions } from './model/use-contact-actions'
import { ContactDetails } from './ui/contact-details'
import { ContactList } from './ui/contact-list'

/** Contacts: the list on the left, the picked contact on the right. */
const { t } = useI18n()

const contacts = useQuery({ queryKey: ['contacts'], queryFn: () => call<Contact[]>('list_contacts') })
const balances = useQuery({ queryKey: ['contacts', 'balances'], queryFn: () => call<ContactBalance[]>('get_contact_balances') })
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })

const search = ref('')
const filter = ref<ContactDebtFilter>('all')
const selectedId = ref('')

const list = computed(() => filterContacts(contacts.data.value ?? [], balances.data.value ?? [], search.value, filter.value))
const sections = computed(() => contactSections(list.value, (kind) => t(`contacts.section.${kind}`)))
/** The picked contact, or the first one shown so the right side is never blank. */
const selected = computed(() => list.value.find((contact) => contact.id === selectedId.value) ?? list.value[0] ?? null)

/* --- a transfer or a debt operation with the selected contact --- */
const transferOpen = ref(false)
const debtEffect = ref<DebtEffect | null>(null)
/** Operations with a contact start from the first everyday wallet, changeable in the form. */
const transferStore = computed(() => {
  const nodes = flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived)
  return (nodes.find((node) => node.store.purpose === 'everyday') ?? nodes[0])?.store.id ?? ''
})
const counterpart = computed<AccountRef | null>(() =>
  selected.value ? { contact: { contact_id: selected.value.id, account_id: null } } : null,
)
function openTransfer(effect: DebtEffect | null) {
  debtEffect.value = effect
  transferOpen.value = true
}

const contactActions = useContactActions(selectedId, transferStore, openTransfer)
</script>

<template>
  <div class="grid items-start gap-4 @container @2xl:grid-cols-[18rem_1fr]">
    <ContactList
      v-model:search="search"
      v-model:filter="filter"
      :sections="sections"
      :total="contacts.data.value?.length ?? 0"
      :balances="balances.data.value ?? []"
      :loaded="contacts.isSuccess.value"
      :selected-id="selected?.id ?? null"
      :actions-of="contactActions"
      @select="(id) => (selectedId = id)"
    />

    <ContactDetails
      v-if="selected"
      :contact="selected"
      :balances="balances.data.value ?? []"
      :can-transfer="Boolean(transferStore)"
      @transfer="openTransfer"
      @deleted="selectedId = ''"
    />
    <EmptyState v-else-if="contacts.isSuccess.value" :title="t('contacts.pickOne')" :description="t('contacts.pickOneHint')" />

    <TransactionCreateDialog
      v-if="transferStore"
      v-model:open="transferOpen"
      :store-id="transferStore"
      :counterpart="counterpart"
      :debt="debtEffect"
    />
  </div>
</template>
