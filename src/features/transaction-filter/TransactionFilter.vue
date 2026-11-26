<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { watchDebounced } from '@vueuse/core'
import { Search } from 'lucide-vue-next'

import { pluralForm } from '@/shared/lib/format'
import { activeFilterCount, emptyTxFilter } from '@/shared/lib/tx-filter'
import { FilterBar } from '@/shared/ui/filter-bar'
import { Input } from '@/shared/ui/input'

import { useFilterDraft } from './model/use-filter-draft'
import { FilterChips } from './ui/filter-chips'
import { FilterFields } from './ui/filter-fields'
import type { TransactionFilterProps } from './transaction-filter.types'

/**
 * Search in the row, everything else in the filter sheet: kinds, an amount range and the switches,
 * applied by `Show N`. Set filters stay visible as chips under the row; each takes itself off.
 */
const props = withDefaults(defineProps<TransactionFilterProps>(), { idPrefix: 'tx-filter' })
/** The count on the right of the row. */
defineSlots<{ trail?(): unknown }>()
const filter = defineModel<TxFilter>({ required: true })

const { t, locale } = useI18n()
const open = ref(false)
const { draft, total, apply } = useFilterDraft(filter, open, () => ['text'])

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

const active = computed(() => activeFilterCount(filter.value))
const applyLabel = computed(() => (total.value === undefined ? t('filter.show') : t('filter.showN', { n: total.value })))
const applyLabelFull = computed(() =>
  total.value === undefined ? t('filter.show') : t(`filter.showOps.${pluralForm(total.value, locale.value)}`, { count: total.value }),
)

function clear() {
  draft.value = emptyTxFilter({ text: filter.value.text })
}
</script>

<template>
  <FilterBar
    v-model:open="open"
    :active="active"
    :apply-label="applyLabel"
    :apply-label-full="applyLabelFull"
    @reset="filter = emptyTxFilter({ text: filter.text })"
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
          :placeholder="t('filter.searchHint')"
          :aria-label="t('filter.search')"
        />
      </div>
    </template>
    <template #below><FilterChips v-model="filter" /></template>
    <template #more><FilterFields v-model="draft" :id-prefix="`${props.idPrefix}-more`" /></template>
    <template v-if="$slots.trail" #trail><slot name="trail" /></template>
  </FilterBar>
</template>
