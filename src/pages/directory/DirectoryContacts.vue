<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Archive, ArchiveRestore, ArrowDownLeft, ArrowRightLeft, ArrowUpRight, Copy, Search } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { ContactArchive, ContactEditor, ContactQuickCreate } from '@/features/contact-edit'
import { DebtPanel } from '@/features/debt-manage'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { ContactTurnoverTable } from '@/entities/contact'
import { TransactionsList } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { Input } from '@/shared/ui/input'
import { Tabs } from '@/shared/ui/tabs'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { formatMoney } from '@/shared/lib/format'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { txDebt } from '@/shared/lib/transactions'
import { emptyTxFilter } from '@/shared/lib/tx-filter'
import { cn } from '@/shared/lib/utils'

const { t } = useI18n()

const contacts = useQuery({ queryKey: ['contacts'], queryFn: () => call<Contact[]>('list_contacts') })
const balances = useQuery({
  queryKey: ['contacts', 'balances'],
  queryFn: () => call<ContactBalance[]>('get_contact_balances'),
})
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })

const search = ref('')
const filter = ref<'all' | 'owe_me' | 'i_owe'>('all')
const selectedId = ref('')

/** Net debt per contact and asset: positive = they owe me. */
function netOf(contactId: string): { symbol: string; net: number }[] {
  return (balances.data.value ?? [])
    .filter((row) => row.contact_id === contactId)
    .map((row) => ({ symbol: row.asset.symbol, net: Number(row.receivable) - Number(row.payable) }))
    .filter((row) => row.net !== 0)
}

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
    .filter((contact) => {
      if (filter.value === 'all') return true
      const nets = netOf(contact.id)
      return filter.value === 'owe_me' ? nets.some((row) => row.net > 0) : nets.some((row) => row.net < 0)
    })
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

const history = useQuery({
  queryKey: computed(() => ['transactions', 'contact', selected.value?.id]),
  queryFn: () =>
    call<Transaction[]>('list_transactions', { filter: emptyTxFilter({ contact_id: selected.value?.id ?? null }) }),
  enabled: computed(() => Boolean(selected.value)),
})
const debtHistory = computed(() => (history.data.value ?? []).filter((tx) => txDebt(tx) !== null))

const detailTab = ref('debts')
const detailTabs = computed(() => [
  { value: 'debts', label: t('contacts.tab.debts') },
  { value: 'history', label: t('contacts.tab.history') },
  { value: 'turnover', label: t('contacts.tab.turnover') },
])

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

/* --- right-click a contact in the list: the common operations without selecting it first --- */
const queryClient = useQueryClient()
const archive = useMutation({
  mutationFn: (contact: Contact) => call('archive_contact', { id: contact.id, archived: !contact.archived }),
  onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['contacts'] }),
  onError: (err) => toast.error(t('contacts.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

async function copy(value: string) {
  try {
    await navigator.clipboard.writeText(value)
    toast.success(t('contacts.copied'))
  } catch {
    toast.error(t('contacts.copyFailed'))
  }
}

function contactActions(contact: Contact): UiAction[] {
  const run = (effect: DebtEffect | null) => () => {
    selectedId.value = contact.id
    openTransfer(effect)
  }
  const blocked = contact.archived ? t('contacts.archivedHint') : !transferStore.value ? t('contacts.noWallet') : false
  return [
    { id: 'transfer', label: t('contacts.writeTransfer'), icon: ArrowRightLeft, disabled: blocked, run: run(null) },
    { id: 'lend', label: t('debt.action.lend'), icon: ArrowUpRight, disabled: blocked, run: run('lend') },
    { id: 'borrow', label: t('debt.action.borrow'), icon: ArrowDownLeft, disabled: blocked, run: run('borrow') },
    {
      id: 'copy',
      label: t('contacts.copyAccount'),
      icon: Copy,
      separated: true,
      hidden: !contact.accounts.length,
      children: contact.accounts.map((account, index) => ({
        id: `copy-${account.id || index}`,
        label: `${account.label || t(`detailKind.${account.kind}`)} · ${account.value}`,
        run: () => void copy(account.value),
      })),
    },
    {
      id: 'archive',
      label: contact.archived ? t('stores.unarchive') : t('stores.archive'),
      icon: contact.archived ? ArchiveRestore : Archive,
      destructive: !contact.archived,
      separated: true,
      run: () => archive.mutate(contact),
    },
  ]
}
</script>

<template>
  <div class="grid items-start gap-4 @container @2xl:grid-cols-[18rem_1fr]">
    <aside class="grid gap-2">
      <ContactQuickCreate @created="(contact) => (selectedId = contact.id)" />
      <div class="relative">
        <Search class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input v-model="search" class="h-9 pl-8" :placeholder="t('contacts.search')" :aria-label="t('contacts.search')" />
      </div>
      <ToggleGroup v-model="filter" type="single" variant="outline" size="sm" class="w-full" :aria-label="t('contacts.filter')">
        <ToggleGroupItem value="all" class="flex-1">{{ t('contacts.filterAll') }}</ToggleGroupItem>
        <ToggleGroupItem value="owe_me" class="flex-1">{{ t('contacts.filterOweMe') }}</ToggleGroupItem>
        <ToggleGroupItem value="i_owe" class="flex-1">{{ t('contacts.filterIOwe') }}</ToggleGroupItem>
      </ToggleGroup>

      <p v-if="contacts.isSuccess.value && !list.length" class="px-1 py-4 text-sm text-muted-foreground">
        {{ contacts.data.value?.length ? t('contacts.nothingFound') : t('contacts.empty') }}
      </p>
      <ul class="grid gap-0.5">
        <li v-for="contact in list" :key="contact.id">
          <ContextMenu :actions="contactActions(contact)">
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
            <span class="flex shrink-0 flex-col items-end text-xs tabular-nums">
              <span
                v-for="row in netOf(contact.id)"
                :key="row.symbol"
                :class="row.net > 0 ? 'text-emerald-600 dark:text-emerald-400' : 'text-destructive'"
              >
                {{ row.net > 0 ? '+' : '−' }}{{ formatMoney(Math.abs(row.net), { decimals: 0, currency: row.symbol }) }}
              </span>
            </span>
          </button>
          </ContextMenu>
        </li>
      </ul>
    </aside>

    <div v-if="selected" class="grid gap-4">
      <Card :class="selected.archived && 'opacity-70'">
        <template #header>
          <div class="flex items-center justify-end gap-1">
            <Button
              variant="outline"
              size="sm"
              :disabled="!transferStore || selected.archived"
              @click="openTransfer(null)"
            >
              <ArrowRightLeft class="size-4" />
              {{ t('contacts.writeTransfer') }}
            </Button>
            <ContactArchive :contact="selected" @deleted="selectedId = ''" />
          </div>
        </template>
        <ContactEditor :key="selected.id" :contact="selected" />
      </Card>

      <Card>
        <Tabs v-model="detailTab" :tabs="detailTabs">
          <template #debts>
            <div class="grid gap-4 @container">
              <DebtPanel :contact="selected" :balances="selectedBalances" @operation="openTransfer" />
              <TransactionsList v-if="debtHistory.length" :transactions="debtHistory" />
            </div>
          </template>
          <template #history>
            <TransactionsList v-if="history.data.value?.length" :transactions="history.data.value" />
            <p v-else class="text-sm text-muted-foreground">{{ t('contacts.noHistory') }}</p>
          </template>
          <template #turnover>
            <ContactTurnoverTable :turnover="turnover.data.value ?? null" />
          </template>
        </Tabs>
      </Card>
    </div>

    <TransactionCreateDialog
      v-if="transferStore"
      v-model:open="transferOpen"
      :store-id="transferStore"
      :counterpart="counterpart"
      :debt="debtEffect"
    />
  </div>
</template>
