<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'

import type { AccountRefPickerProps } from './account-ref-picker.types'

const props = withDefaults(defineProps<AccountRefPickerProps>(), {
  stores: true,
  contacts: true,
  raw: true,
  excludeStoreId: null,
  emptyLabel: '',
})

const side = defineModel<AccountRef | null>({ default: null })

const { t } = useI18n()

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

/** One `<option>` value per side: `store:<id>`, `contact:<id>`, `contact:<id>:<account>`, `raw`. */
function encode(value: AccountRef | null): string {
  if (!value) return ''
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
  if (encode(value) !== choice.value) choice.value = encode(value)
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

const storeOptions = computed(() =>
  flattenStoreTree(tree.data.value ?? [])
    .filter((node) => !node.store.archived && node.store.id !== props.excludeStoreId)
    .map((node) => ({ value: `store:${node.store.id}`, label: node.path.join(' › ') })),
)

/** A contact, then each of its active accounts with the value, so the browser's type-ahead finds a card number. */
const contactOptions = computed(() =>
  (contactList.data.value ?? [])
    .filter((contact) => !contact.archived)
    .flatMap((contact) => [
      { value: `contact:${contact.id}`, label: contact.name },
      ...contact.accounts
        .filter((account) => account.active)
        .map((account) => ({
          value: `contact:${contact.id}:${account.id}`,
          label: `${contact.name} › ${account.label || t(`detailKind.${account.kind}`)} · ${account.value}`,
        })),
    ]),
)
</script>

<template>
  <div class="grid gap-1.5">
    <Select v-model="choice">
      <option v-if="props.emptyLabel" value="">{{ props.emptyLabel }}</option>
      <option v-else value="" disabled>{{ t('tx.field.pickSide') }}</option>
      <optgroup v-if="props.stores && storeOptions.length" :label="t('tx.side.myWallets')">
        <option v-for="option in storeOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
      </optgroup>
      <optgroup v-if="props.contacts && contactOptions.length" :label="t('tx.side.contacts')">
        <option v-for="option in contactOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
      </optgroup>
      <option v-if="props.raw" :value="RAW">{{ t('tx.side.other') }}</option>
    </Select>
    <Input v-if="choice === RAW" v-model="rawText" :placeholder="t('tx.side.otherHint')" />
  </div>
</template>
