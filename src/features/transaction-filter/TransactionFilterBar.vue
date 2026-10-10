<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { watchDebounced } from '@vueuse/core'
import { Link2, X } from 'lucide-vue-next'

import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree, storeDescendants } from '@/shared/lib/store-tree'
import { TX_KINDS } from '@/shared/lib/transactions'
import { activeFilterCount, emptyTxFilter } from '@/shared/lib/tx-filter'

import type { TransactionFilterBarProps } from './transaction-filter.types'

const props = withDefaults(defineProps<TransactionFilterBarProps>(), { fixedScope: false, idPrefix: 'tx-filter' })
const filter = defineModel<TxFilter>({ required: true })

const { t } = useI18n()
const field = (name: string) => `${props.idPrefix}-${name}`

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const contacts = useQuery({ queryKey: ['contacts'], queryFn: () => call<Contact[]>('list_contacts') })
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const storeId = computed({
  get: () => filter.value.scope.store_ids[0] ?? '',
  set: (id: string) => {
    filter.value = { ...filter.value, scope: { store_ids: id ? [id] : [], include_children: true } }
  },
})
/** Sub-accounts the picked wallet has: the «+N» next to «with sub-accounts». */
const childCount = computed(() => {
  const node = nodes.value.find((entry) => entry.store.id === storeId.value)
  return node ? storeDescendants(node).length : 0
})
const includeChildren = computed({
  get: () => filter.value.scope.include_children,
  set: (value: boolean) => {
    filter.value = { ...filter.value, scope: { ...filter.value.scope, include_children: value } }
  },
})

/** One kind at a time in the picker; the filter itself takes a list. */
const kind = computed({
  get: () => filter.value.kinds[0] ?? '',
  set: (value: string) => {
    filter.value = { ...filter.value, kinds: value ? [value as TxKind] : [] }
  },
})

/** Text, date and select inputs hold `''` for «not set»; the filter holds `null`. */
const nullable = (name: 'contact_id' | 'from' | 'to') =>
  computed({
    get: () => filter.value[name] ?? '',
    set: (value: string) => {
      filter.value = { ...filter.value, [name]: value || null }
    },
  })
const contactId = nullable('contact_id')
const from = nullable('from')
const to = nullable('to')

const hideInternal = computed({
  get: () => filter.value.hide_internal,
  set: (value: boolean) => {
    filter.value = { ...filter.value, hide_internal: value }
  },
})

// Typing does not refetch on every key.
const search = ref(filter.value.text ?? '')
watchDebounced(
  search,
  (value) => {
    filter.value = { ...filter.value, text: value.trim() || null }
  },
  { debounce: 300 },
)
watch(
  () => filter.value.text,
  (value) => {
    if ((value ?? '') !== search.value.trim()) search.value = value ?? ''
  },
)

const active = computed(() => activeFilterCount(filter.value) - (props.fixedScope && storeId.value ? 1 : 0))
function reset() {
  const kept = props.fixedScope ? { scope: filter.value.scope } : {}
  filter.value = emptyTxFilter(kept)
}
</script>

<template>
  <div class="flex flex-wrap items-end gap-3">
    <div v-if="!props.fixedScope" class="grid gap-1.5">
      <Label :for="field('scope')">{{ t('filter.scope') }}</Label>
      <StoreNodeSelect :id="field('scope')" v-model="storeId" :nodes="nodes" :empty-label="t('tx.allStores')" class="w-56" />
    </div>
    <label v-if="!props.fixedScope && storeId && childCount" class="flex h-9 items-center gap-2 text-sm">
      <input v-model="includeChildren" type="checkbox" class="size-4 accent-primary" />
      {{ t('filter.withChildren', { count: childCount }) }}
    </label>
    <div class="grid gap-1.5">
      <Label :for="field('kind')">{{ t('form.kind') }}</Label>
      <Select :id="field('kind')" v-model="kind" class="w-40">
        <option value="">{{ t('hist.allKinds') }}</option>
        <option v-for="entry in TX_KINDS" :key="entry" :value="entry">{{ t(`tx.kind.${entry}`) }}</option>
      </Select>
    </div>
    <div v-if="contacts.data.value?.length" class="grid gap-1.5">
      <Label :for="field('contact')">{{ t('filter.contact') }}</Label>
      <Select :id="field('contact')" v-model="contactId" class="w-44">
        <option value="">{{ t('filter.anyContact') }}</option>
        <option v-for="contact in contacts.data.value" :key="contact.id" :value="contact.id">{{ contact.name }}</option>
      </Select>
    </div>
    <div class="grid gap-1.5">
      <Label :for="field('from')">{{ t('filter.from') }}</Label>
      <Input :id="field('from')" v-model="from" type="date" class="w-40" />
    </div>
    <div class="grid gap-1.5">
      <Label :for="field('to')">{{ t('filter.to') }}</Label>
      <Input :id="field('to')" v-model="to" type="date" class="w-40" />
    </div>
    <div class="grid gap-1.5">
      <Label :for="field('text')">{{ t('filter.search') }}</Label>
      <Input :id="field('text')" v-model="search" class="w-52" :placeholder="t('hist.search')" />
    </div>
    <label class="flex h-9 items-center gap-2 text-sm">
      <input v-model="hideInternal" type="checkbox" class="size-4 accent-primary" />
      {{ t('filter.hideInternal') }}
    </label>
    <Button
      v-if="filter.flow_id"
      variant="secondary"
      size="sm"
      class="h-9"
      :title="t('filter.clearFlow')"
      @click="filter = { ...filter, flow_id: null }"
    >
      <Link2 class="size-4" />
      {{ t('filter.flow') }}
      <X class="size-3.5" />
    </Button>
    <Button v-if="active > 0" variant="ghost" size="sm" class="h-9" @click="reset">
      {{ t('filter.reset', { count: active }) }}
    </Button>
  </div>
</template>
