<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Building2, Check, ChevronDown, CreditCard, Hash, Landmark, Link, PenLine, Phone, User, Wallet } from 'lucide-vue-next'
import {
  ComboboxAnchor,
  ComboboxContent,
  ComboboxInput,
  ComboboxItem,
  ComboboxPortal,
  ComboboxRoot,
  ComboboxTrigger,
  ComboboxViewport,
} from 'reka-ui'

import { Input } from '@/shared/ui/input'
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

import type { AccountOption, AccountRefPickerProps } from './account-ref-picker.types'

/**
 * Who the money moves with, picked like a currency: a searchable list where
 * each row says what it is on top and the details under it — a wallet's path
 * and balance, a contact's card or address. Every side appears exactly once.
 */
const props = withDefaults(defineProps<AccountRefPickerProps>(), {
  stores: true,
  contacts: true,
  raw: true,
  excludeStoreId: null,
  emptyLabel: '',
  id: '',
})

const side = defineModel<AccountRef | null>({ default: null })

const { t } = useI18n()
const settings = useSettingsStore()
const generatedId = useId()
const triggerId = computed(() => props.id || generatedId)

const tree = useQuery({
  queryKey: STORE_TREE_QUERY_KEY,
  queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
  enabled: computed(() => props.stores),
})
const contactList = useQuery({
  queryKey: ['contacts'],
  queryFn: () => call<Contact[]>('list_contacts'),
  enabled: computed(() => props.contacts),
})

const RAW = 'raw'
/** The «not set» row. Reka throws on an item whose value is `''`, which left the list half-rendered. */
const NONE = 'none'

function encode(value: AccountRef | null): string {
  if (!value) return NONE
  if ('store' in value) return `store:${value.store}`
  if ('contact' in value) {
    const { contact_id: contactId, account_id: accountId } = value.contact
    return accountId ? `contact:${contactId}:${accountId}` : `contact:${contactId}`
  }
  return RAW
}

const rawText = ref(side.value && 'raw' in side.value ? side.value.raw : '')
const choice = ref(encode(side.value))

// Follow outside changes (a swap of the two ends, an edit loaded later).
watch(side, (value) => {
  // Free text not typed yet is still a free-text choice, not «not set».
  const keepRaw = value === null && choice.value === RAW
  if (!keepRaw && encode(value) !== choice.value) choice.value = encode(value)
  if (value && 'raw' in value && value.raw !== rawText.value) rawText.value = value.raw
})

function commit() {
  const [kind, id, account] = choice.value.split(':')
  if (kind === 'store' && id) side.value = { store: id }
  else if (kind === 'contact' && id) side.value = { contact: { contact_id: id, account_id: account ?? null } }
  else if (choice.value === RAW) side.value = rawText.value.trim() ? { raw: rawText.value.trim() } : null
  else side.value = null
}
watch(choice, commit)
watch(rawText, () => {
  if (choice.value === RAW) commit()
})

const storeOptions = computed<AccountOption[]>(() =>
  flattenStoreTree(tree.data.value ?? [])
    .filter((node) => !node.store.archived && node.store.id !== props.excludeStoreId)
    .map((node) => {
      const parent = node.path.slice(0, -1).join(' › ')
      const binding = typeof node.store.binding === 'object' && 'asset' in node.store.binding ? node.store.binding.asset.symbol : ''
      const balance = formatMoney(node.subtree_base, { currency: settings.baseCurrency })
      const detail = [parent || t(`stores.kind.${node.store.kind}`), binding, balance].filter(Boolean).join(' · ')
      return {
        value: `store:${node.store.id}`,
        title: node.store.name,
        detail,
        group: t('tx.side.myWallets'),
        haystack: `${node.path.join(' ')} ${binding}`.toLowerCase(),
      }
    }),
)

/** A contact, then each of its active accounts: the card number is searchable. */
const contactOptions = computed<AccountOption[]>(() =>
  (contactList.data.value ?? [])
    .filter((contact) => !contact.archived)
    .sort((left, right) => left.group.localeCompare(right.group) || left.name.localeCompare(right.name))
    .flatMap((contact) => {
      const group = contact.group.trim() || t('tx.side.contacts')
      const accounts = contact.accounts.filter((account) => account.active)
      return [
        {
          value: `contact:${contact.id}`,
          title: contact.name,
          detail: accounts.length
            ? t('tx.side.contactAccounts', { count: accounts.length })
            : t(`contactKind.${contact.kind}`),
          group,
          haystack: `${contact.name} ${contact.group}`.toLowerCase(),
        },
        ...accounts.map((account) => ({
          value: `contact:${contact.id}:${account.id}`,
          title: account.label || t(`detailKind.${account.kind}`),
          detail: [account.value, account.currency?.toUpperCase() ?? ''].filter(Boolean).join(' · '),
          group,
          haystack: `${contact.name} ${account.label} ${account.value} ${account.currency ?? ''}`.toLowerCase(),
          parent: `contact:${contact.id}`,
          detailKind: account.kind,
        })),
      ]
    }),
)

const allOptions = computed<AccountOption[]>(() => {
  const rows = [...(props.stores ? storeOptions.value : []), ...(props.contacts ? contactOptions.value : [])]
  if (props.raw) {
    rows.push({ value: RAW, title: t('tx.side.other'), detail: t('tx.side.otherHint'), group: '', haystack: '' })
  }
  // One row per side: the same wallet or account never shows twice.
  const seen = new Set<string>()
  return rows.filter((row) => !seen.has(row.value) && seen.add(row.value))
})

const open = ref(false)
const search = ref('')
watch(open, (isOpen) => {
  if (isOpen) search.value = ''
})
/** A matching account keeps its contact on top, so it is clear whose card it is. */
const shown = computed(() => {
  const needle = search.value.trim().toLowerCase()
  if (!needle) return allOptions.value
  const hit = (row: AccountOption) => row.value === RAW || row.haystack.includes(needle)
  const parents = new Set(allOptions.value.filter((row) => row.parent && hit(row)).map((row) => row.parent))
  return allOptions.value.filter((row) => hit(row) || parents.has(row.value))
})
/** Rows under their headings, in first-seen order. */
const groups = computed(() => {
  const out = new Map<string, AccountOption[]>()
  for (const row of shown.value) out.set(row.group, [...(out.get(row.group) ?? []), row])
  return [...out.entries()]
})

const current = computed(() => allOptions.value.find((row) => row.value === choice.value) ?? null)
const DETAIL_ICONS: Record<DetailKind, typeof User> = {
  card: CreditCard,
  crypto_address: Wallet,
  iban: Landmark,
  account: Landmark,
  phone: Phone,
  url: Link,
  other: Hash,
}
function iconOf(row: AccountOption) {
  if (row.detailKind) return DETAIL_ICONS[row.detailKind]
  return row.value.startsWith('store:') ? Building2 : row.value === RAW ? PenLine : User
}
</script>

<template>
  <div class="grid gap-1.5">
    <ComboboxRoot v-model="choice" v-model:open="open" :ignore-filter="true" open-on-click>
      <ComboboxAnchor as-child>
        <ComboboxTrigger as-child :tabindex="0" :aria-label="t('tx.field.pickSide')">
          <button
            :id="triggerId"
            type="button"
            :aria-expanded="open"
            class="flex min-h-9 w-full min-w-0 items-center justify-between gap-2 rounded-md border border-input bg-transparent px-3 py-1 text-left text-sm shadow-sm transition-colors hover:bg-accent data-[state=open]:border-primary"
          >
            <span v-if="current" class="grid min-w-0 leading-tight">
              <span class="truncate font-medium">{{ current.title }}</span>
              <span class="truncate text-xs text-muted-foreground">{{ current.detail }}</span>
            </span>
            <span v-else class="truncate text-muted-foreground">
              {{ props.emptyLabel || t('tx.field.pickSide') }}
            </span>
            <ChevronDown class="size-4 shrink-0 text-muted-foreground" />
          </button>
        </ComboboxTrigger>
      </ComboboxAnchor>

      <ComboboxPortal>
        <ComboboxContent
          position="popper"
          class="z-[60] w-[var(--reka-combobox-trigger-width)] min-w-72 overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-lg"
          :side-offset="4"
        >
          <div class="border-b border-border p-1.5">
            <ComboboxInput
              v-model="search"
              auto-focus
              autocomplete="off"
              spellcheck="false"
              :display-value="() => search"
              :placeholder="t('tx.side.search')"
              class="h-8 w-full rounded border border-input bg-transparent px-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            />
          </div>
          <ComboboxViewport class="max-h-80 overflow-y-auto p-1">
            <ComboboxItem
              v-if="props.emptyLabel && !search.trim()"
              :value="NONE"
              class="flex w-full cursor-pointer select-none items-center gap-2 rounded px-2 py-1.5 text-sm text-muted-foreground outline-none data-[highlighted]:bg-accent"
            >
              {{ props.emptyLabel }}
            </ComboboxItem>
            <template v-for="[group, rows] in groups" :key="group">
              <p v-if="group" class="px-2 pb-0.5 pt-2 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                {{ group }}
              </p>
              <!-- A contact's accounts hang under it, indented, instead of repeating its name. -->
              <ComboboxItem
                v-for="row in rows"
                :key="row.value"
                :value="row.value"
                class="flex w-full cursor-pointer select-none items-center gap-2 rounded px-2 py-1.5 text-sm outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
                :class="row.parent && 'ml-4 w-[calc(100%-1rem)] rounded-l-none border-l border-border py-1'"
              >
                <Check v-if="row.value === choice" class="size-3.5 shrink-0 text-primary" aria-hidden="true" />
                <component :is="iconOf(row)" v-else class="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                <span class="grid min-w-0 leading-tight">
                  <span class="truncate" :class="row.parent ? 'text-[13px]' : 'font-medium'">{{ row.title }}</span>
                  <span class="truncate text-xs text-muted-foreground">{{ row.detail }}</span>
                </span>
              </ComboboxItem>
            </template>
            <p v-if="!shown.length" class="px-2 py-3 text-center text-xs text-muted-foreground">{{ t('tx.side.noMatches') }}</p>
          </ComboboxViewport>
        </ComboboxContent>
      </ComboboxPortal>
    </ComboboxRoot>
    <Input v-if="choice === RAW" v-model="rawText" :placeholder="t('tx.side.otherHint')" />
  </div>
</template>
