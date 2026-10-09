<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ArrowRightLeft, Pencil, Plus } from 'lucide-vue-next'

import { ContactArchive, ContactDialog } from '@/features/contact-edit'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { ContactCard } from '@/entities/contact'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { cn } from '@/shared/lib/utils'

const { t } = useI18n()

const contacts = useQuery({ queryKey: ['contacts'], queryFn: () => call<Contact[]>('list_contacts') })
const balances = useQuery({
  queryKey: ['contacts', 'balances'],
  queryFn: () => call<ContactBalance[]>('get_contact_balances'),
})
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })

const search = ref('')
const selectedId = ref('')

/** Matches the name and any account value, so a card number finds its owner. */
const list = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return (contacts.data.value ?? [])
    .filter(
      (contact) =>
        !needle ||
        contact.name.toLowerCase().includes(needle) ||
        contact.accounts.some((account) => account.value.toLowerCase().includes(needle)),
    )
    .sort((left, right) => Number(left.archived) - Number(right.archived) || left.name.localeCompare(right.name))
})

const selected = computed(() => list.value.find((contact) => contact.id === selectedId.value) ?? list.value[0] ?? null)
const selectedBalances = computed(() =>
  (balances.data.value ?? []).filter((row) => row.contact_id === selected.value?.id),
)

const turnover = useQuery({
  queryKey: computed(() => ['contacts', 'turnover', selected.value?.id]),
  queryFn: () =>
    call<ContactTurnover>('get_contact_turnover', { input: { contact_id: selected.value?.id, account_id: null } }),
  enabled: computed(() => Boolean(selected.value)),
})

/** Net debt per contact, for the list badges. */
function owes(contactId: string): string {
  const rows = (balances.data.value ?? []).filter((row) => row.contact_id === contactId)
  return rows
    .map((row) => {
      const net = Number(row.receivable) - Number(row.payable)
      return net ? `${net > 0 ? '+' : ''}${net} ${row.asset.symbol}` : ''
    })
    .filter(Boolean)
    .join(' · ')
}

const createOpen = ref(false)
const editOpen = ref(false)
const transferOpen = ref(false)

/** A transfer to a contact starts from the first wallet in the budget, changeable in the form. */
const transferStore = computed(() => {
  const nodes = flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived)
  return (nodes.find((node) => node.store.in_budget) ?? nodes[0])?.store.id ?? ''
})
const transferTo = computed<AccountRef | null>(() =>
  selected.value ? { contact: { contact_id: selected.value.id, account_id: null } } : null,
)
</script>

<template>
  <div class="grid gap-4 @container">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <Input v-model="search" class="h-9 w-64" :placeholder="t('contacts.search')" />
      <Button @click="createOpen = true">
        <Plus class="size-4" />
        {{ t('contacts.new') }}
      </Button>
    </div>

    <div
      v-if="contacts.isSuccess.value && !contacts.data.value?.length"
      class="flex flex-col items-center gap-2 rounded-xl border border-dashed border-border p-8 text-center"
    >
      <p class="text-sm text-muted-foreground">{{ t('contacts.empty') }}</p>
      <Button class="mt-2" @click="createOpen = true">
        <Plus class="size-4" />
        {{ t('contacts.new') }}
      </Button>
    </div>

    <div v-else class="grid items-start gap-4 @2xl:grid-cols-[18rem_1fr]">
      <ul class="grid gap-1">
        <li v-for="contact in list" :key="contact.id">
          <button
            type="button"
            :class="
              cn(
                'flex w-full items-center justify-between gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
                selected?.id === contact.id && 'bg-accent',
                contact.archived && 'opacity-60',
              )
            "
            @click="selectedId = contact.id"
          >
            <span class="min-w-0 truncate font-medium">{{ contact.name }}</span>
            <span class="shrink-0 text-xs tabular-nums text-muted-foreground">{{ owes(contact.id) }}</span>
          </button>
        </li>
      </ul>

      <ContactCard
        v-if="selected"
        :contact="selected"
        :balances="selectedBalances"
        :turnover="turnover.data.value ?? null"
      >
        <template #actions>
          <Button
            variant="ghost"
            size="icon"
            class="size-8 text-muted-foreground"
            :title="t('contacts.writeTransfer')"
            :aria-label="t('contacts.writeTransfer')"
            :disabled="!transferStore || selected.archived"
            @click="transferOpen = true"
          >
            <ArrowRightLeft class="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            class="size-8 text-muted-foreground"
            :title="t('contacts.edit')"
            :aria-label="t('contacts.edit')"
            @click="editOpen = true"
          >
            <Pencil class="size-4" />
          </Button>
          <ContactArchive :contact="selected" @deleted="selectedId = ''" />
        </template>
      </ContactCard>
    </div>

    <ContactDialog v-model:open="createOpen" @saved="(contact) => (selectedId = contact.id)" />
    <ContactDialog v-model:open="editOpen" :contact="selected" />
    <TransactionCreateDialog
      v-if="transferStore"
      v-model:open="transferOpen"
      :store-id="transferStore"
      :counterpart="transferTo"
    />
  </div>
</template>
