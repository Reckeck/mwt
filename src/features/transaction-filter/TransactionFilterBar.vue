<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { watchDebounced } from '@vueuse/core'
import { Link2, X } from 'lucide-vue-next'

import { CategorySelect } from '@/features/category-select'
import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { FILTER_FIELD_CLASS, FilterBar } from '@/shared/ui/filter-bar'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { flattenStoreTree, storeDescendants } from '@/shared/lib/store-tree'
import { TX_KINDS } from '@/shared/lib/transactions'
import { activeFilterCount, emptyTxFilter } from '@/shared/lib/tx-filter'
import { Checkbox } from '@/shared/ui/checkbox'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import type { TransactionFilterBarProps } from './transaction-filter.types'
import { useContacts, useStoreTree } from '@/shared/api/queries'

const props = withDefaults(defineProps<TransactionFilterBarProps>(), { fixedScope: false, idPrefix: 'tx-filter' })
/** Things that belong to the row but are not filters (refresh, export). */
defineSlots<{ actions?(): unknown }>()
const filter = defineModel<TxFilter>({ required: true })

const { t } = useI18n()
const field = (name: string) => `${props.idPrefix}-${name}`

const tree = useStoreTree()
const contacts = useContacts()
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const storeIds = computed({
  get: () => filter.value.scope.store_ids,
  set: (ids: string[]) => {
    filter.value = { ...filter.value, scope: { store_ids: ids, include_children: filter.value.scope.include_children } }
  },
})
/** Sub-accounts the picked wallets have: the «+N» next to «with sub-accounts». */
const childCount = computed(() =>
  nodes.value
    .filter((entry) => storeIds.value.includes(entry.store.id))
    .reduce((sum, node) => sum + storeDescendants(node).length, 0),
)
const includeChildren = computed({
  get: () => filter.value.scope.include_children,
  set: (value: boolean) => {
    filter.value = { ...filter.value, scope: { ...filter.value.scope, include_children: value } }
  },
})

/** One kind at a time in the picker; several (from a report) show as a chip instead. */
const kind = computed({
  get: () => (filter.value.kinds.length === 1 ? filter.value.kinds[0] : ''),
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

const category = computed({
  get: () => filter.value.category ?? '',
  set: (value: string | string[]) => {
    const key = Array.isArray(value) ? (value[0] ?? '') : value
    filter.value = { ...filter.value, category: key || null }
  },
})

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

const active = computed(() => activeFilterCount(filter.value) - (props.fixedScope && storeIds.value.length ? 1 : 0))
/** Set filters among the folded ones. */
const moreActive = computed(
  () =>
    [filter.value.contact_id, filter.value.from || filter.value.to, filter.value.hide_internal, filter.value.category !== null].filter(
      Boolean,
    ).length,
)
function reset() {
  const kept = props.fixedScope ? { scope: filter.value.scope } : {}
  filter.value = emptyTxFilter(kept)
}
</script>

<template>
  <FilterBar :active="active" :more-active="moreActive" @reset="reset">
    <template #lead>
      <div :class="FILTER_FIELD_CLASS">
        <Label :for="field('text')">{{ t('filter.search') }}</Label>
        <Input :id="field('text')" v-model="search" class="w-full @3xl:w-52" :placeholder="t('hist.search')" />
      </div>
    </template>
    <div v-if="!props.fixedScope" :class="FILTER_FIELD_CLASS">
      <Label :for="field('scope')">{{ t('filter.scope') }}</Label>
      <StoreNodeSelect
        :id="field('scope')"
        v-model:ids="storeIds"
        multiple
        :nodes="nodes"
        :empty-label="t('tx.allStores')"
        class="w-full @3xl:w-56"
      />
    </div>
    <label v-if="!props.fixedScope && storeIds.length && childCount" :for="field('children')" class="flex h-9 items-center gap-2 text-sm">
      <Checkbox :id="field('children')" v-model="includeChildren" />
      {{ t('filter.withChildren', { count: childCount }) }}
    </label>
    <div :class="FILTER_FIELD_CLASS">
      <Label :for="field('kind')">{{ t('form.kind') }}</Label>
      <Select :id="field('kind')" v-model="kind" :disabled="filter.kinds.length > 1" class="w-full @3xl:w-40">
        <SelectItem value="">{{ t('hist.allKinds') }}</SelectItem>
        <SelectItem v-for="entry in TX_KINDS" :key="entry" :value="entry">{{ t(`tx.kind.${entry}`) }}</SelectItem>
      </Select>
    </div>
    <!-- What a report opened the list with: each is a chip that takes itself off. -->
    <Button v-if="filter.kinds.length > 1" variant="secondary" class="shrink-0" @click="filter = { ...filter, kinds: [] }">
      {{ t('filter.kindsCount', { n: filter.kinds.length }) }}
      <X class="size-3.5" />
    </Button>
    <Button v-if="filter.with_fee" variant="secondary" class="shrink-0" @click="filter = { ...filter, with_fee: false }">
      {{ t('filter.withFee') }}
      <X class="size-3.5" />
    </Button>
    <Button v-if="filter.realized" variant="secondary" class="shrink-0" @click="filter = { ...filter, realized: false }">
      {{ t('filter.realized') }}
      <X class="size-3.5" />
    </Button>
    <Button
      v-if="filter.flow_id"
      variant="secondary"
      class="shrink-0"
      :title="t('filter.clearFlow')"
      @click="filter = { ...filter, flow_id: null }"
    >
      <Link2 class="size-4" />
      {{ t('filter.flow') }}
      <X class="size-3.5" />
    </Button>

    <!-- Used less often: folded under «Ещё фильтры», with a count of what is set. -->
    <template #more>
      <div v-if="contacts.data.value?.length" class="grid gap-1.5">
        <Label :for="field('contact')">{{ t('filter.contact') }}</Label>
        <Select :id="field('contact')" v-model="contactId">
          <SelectItem value="">{{ t('filter.anyContact') }}</SelectItem>
          <SelectItem v-for="contact in contacts.data.value" :key="contact.id" :value="contact.id">{{ contact.name }}</SelectItem>
        </Select>
      </div>
      <div class="grid gap-1.5">
        <Label :for="field('category')">{{ t('filter.category') }}</Label>
        <CategorySelect
          :id="field('category')"
          v-model="category"
          :groups="['daily', 'fixed', 'income', 'savings']"
          :placeholder="t('filter.anyCategory')"
          no-create
        />
      </div>
      <div class="grid grid-cols-2 gap-2">
        <div class="grid min-w-0 gap-1.5">
          <Label :for="field('from')">{{ t('filter.from') }}</Label>
          <DateTimePicker :id="field('from')" v-model="from" :max="to || undefined" />
        </div>
        <div class="grid min-w-0 gap-1.5">
          <Label :for="field('to')">{{ t('filter.to') }}</Label>
          <DateTimePicker :id="field('to')" v-model="to" :min="from || undefined" />
        </div>
      </div>
      <label :for="field('internal')" class="flex items-center gap-2 text-sm">
        <Checkbox :id="field('internal')" v-model="hideInternal" />
        {{ t('filter.hideInternal') }}
      </label>
    </template>
    <template v-if="$slots.actions" #actions><slot name="actions" /></template>
  </FilterBar>
</template>
