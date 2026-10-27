<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { watchDebounced } from '@vueuse/core'
import { Link2, Search, X } from 'lucide-vue-next'

import { pluralForm } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { activeFilterCount, emptyTxFilter } from '@/shared/lib/tx-filter'
import { Button } from '@/shared/ui/button'
import { FilterBar } from '@/shared/ui/filter-bar'
import { Input } from '@/shared/ui/input'

import { useFilterDraft } from './model/use-filter-draft'
import { FilterSwitches } from './ui/filter-switches'
import { KindField } from './ui/kind-field'
import { MoreFields } from './ui/more-fields'
import { ScopeField } from './ui/scope-field'
import type { TransactionFilterBarProps } from './transaction-filter.types'

/**
 * The operations' filters: search, wallet and kind in the row; period,
 * category, contact, amount and the switches under «Ещё фильтры», applied by
 * «Показать N». On a phone everything but the search is in the «Фильтры» screen.
 */
const props = withDefaults(defineProps<TransactionFilterBarProps>(), { fixedScope: false, idPrefix: 'tx-filter' })
/** Things that belong to the row but are not filters (export), and the count on the right. */
defineSlots<{ actions?(): unknown; trail?(): unknown }>()
const filter = defineModel<TxFilter>({ required: true })

const { t, locale } = useI18n()
const phone = useIsPhone()
const open = ref(false)
/** Filters edited in the row, not in the panel: «Показать» must not overwrite them. A phone has only the search there. */
const ROW_KEYS: (keyof TxFilter)[] = ['text', 'flow_id', 'with_fee', 'realized']
const { draft, total, apply } = useFilterDraft(filter, open, () =>
  phone.value ? ROW_KEYS : [...ROW_KEYS, 'kinds', ...(props.fixedScope ? [] : (['scope'] as const))],
)

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

const kept = () => (props.fixedScope ? { scope: filter.value.scope } : {})
const active = computed(() => activeFilterCount(filter.value) - (props.fixedScope && filter.value.scope.store_ids.length ? 1 : 0))
/** Set filters among the folded ones. */
const moreActive = computed(() => {
  const value = filter.value
  return [
    value.from || value.to,
    value.category !== null,
    value.contact_id,
    value.amount_min || value.amount_max,
    value.auto_only,
    value.debt_only,
    value.hide_internal,
    value.with_attachments,
    value.unpriced,
  ].filter(Boolean).length
})

const applyLabel = computed(() => (total.value === undefined ? t('filter.show') : t('filter.showN', { n: total.value })))
const applyLabelFull = computed(() =>
  total.value === undefined
    ? t('filter.show')
    : t(`filter.showOps.${pluralForm(total.value, locale.value)}`, { count: total.value }),
)

function reset() {
  filter.value = emptyTxFilter(kept())
}
/**
 * «Сбросить» in the panel clears the draft at once, and the count under «Показать» follows. A computer
 * also applies it: the list behind the panel updates. A phone waits for «Показать», which closes the screen.
 */
function clear() {
  draft.value = emptyTxFilter({
    ...(phone.value ? kept() : { scope: filter.value.scope, kinds: filter.value.kinds }),
    text: filter.value.text,
    flow_id: filter.value.flow_id,
    with_fee: filter.value.with_fee,
    realized: filter.value.realized,
  })
  if (!phone.value) apply()
}
</script>

<template>
  <FilterBar
    v-model:open="open"
    :active="active"
    :more-active="moreActive"
    :apply-label="applyLabel"
    :apply-label-full="applyLabelFull"
    @reset="reset"
    @clear="clear"
    @apply="apply"
  >
    <template #lead>
      <div class="relative min-w-40 flex-[1.4] basis-48 @3xl:max-w-[260px]">
        <Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
        <Input
          :id="`${props.idPrefix}-text`"
          v-model="search"
          type="search"
          class="w-full pl-9"
          :placeholder="phone ? t('filter.search') : t('filter.searchHint')"
          :aria-label="t('filter.search')"
        />
      </div>
    </template>

    <ScopeField v-if="!props.fixedScope" v-model="filter" :id-prefix="props.idPrefix" bare />
    <KindField v-model="filter" :id-prefix="props.idPrefix" bare />
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

    <template #more>
      <MoreFields v-model="draft" :id-prefix="`${props.idPrefix}-more`">
        <!-- A phone has no row for them: they follow the period on the «Фильтры» screen. -->
        <template v-if="phone">
          <ScopeField v-if="!props.fixedScope" v-model="draft" :id-prefix="`${props.idPrefix}-more`" />
          <KindField v-model="draft" :id-prefix="`${props.idPrefix}-more`" />
        </template>
      </MoreFields>
    </template>
    <template #switches>
      <FilterSwitches v-model="draft" :id-prefix="`${props.idPrefix}-more`" />
    </template>
    <template v-if="$slots.actions" #actions><slot name="actions" /></template>
    <template v-if="$slots.trail" #trail><slot name="trail" /></template>
  </FilterBar>
</template>
