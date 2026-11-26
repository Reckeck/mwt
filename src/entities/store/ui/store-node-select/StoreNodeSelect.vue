<script setup lang="ts">
import { computed, ref, useAttrs } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { decimalAdd } from '@/shared/lib/decimal'
import { formatMoney } from '@/shared/lib/format'
import { useSettingsStore } from '@/shared/store/settings'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { PickerRow, PickerSection, PickerSheet } from '@/shared/ui/picker-sheet'

import { walletRow, type WalletRow } from '../../lib/wallet-option'

/** The line under the name of an `inline` trigger; a function gets the picked wallet's balance (`отдал · на счёте 52 000 ₽`). */
type StoreCaption = string | ((balance: string) => string)

/**
 * A wallet picked from a bottom sheet on a phone (board 532847), a popover under the trigger on a
 * desktop (`PickerSheet`, board 984555): a search row, then the wallets grouped by what the money is
 * for, each group with its total, each `PickerRow` an icon circle, the name, the bound asset under
 * it and the balance on the right with its `≈ base` under a foreign currency.
 * The trigger is an outline field (`field`), the muted wallet plate of the quick form (`plate`,
 * board 533927), or the bare wallet line inside an exchange box with a caption under the name
 * (`inline`, board 534325). `id`, `aria-*` and `class` go to the trigger.
 * `nodes` is a flat list: a sub-account is listed only when the caller passes it.
 * `emptyLabel` makes the pick optional: a first row with that label clears it.
 */
defineOptions({ inheritAttrs: false })
const props = withDefaults(
  defineProps<{ nodes: StoreTreeNode[]; invalid?: boolean; placeholder?: string; variant?: 'field' | 'plate' | 'inline'; caption?: StoreCaption; emptyLabel?: string }>(),
  { variant: 'field', placeholder: undefined, caption: undefined, emptyLabel: undefined },
)
const model = defineModel<string>({ default: '' })
const attrs = useAttrs()
const { t } = useI18n()
const settings = useSettingsStore()

const PURPOSES: Purpose[] = ['everyday', 'savings', 'investment', 'transit']
const open = ref(false)
const search = ref('')

const entries = computed(() =>
  props.nodes.map((node) => ({
    purpose: node.store.purpose,
    base: node.subtree_base,
    row: walletRow(node, t, settings.baseCurrency),
  })),
)
const current = computed(() => entries.value.find((entry) => entry.row.id === model.value)?.row)
const groups = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return PURPOSES.map((purpose) => {
    const inGroup = entries.value.filter((entry) => entry.purpose === purpose)
    const total = inGroup.reduce((sum, entry) => decimalAdd(sum, entry.base) ?? sum, '0')
    return {
      purpose,
      // money: plain — the total on the right of a group heading
      total: formatMoney(total, { currency: settings.baseCurrency }),
      rows: inGroup
        .filter((entry) => !needle || entry.row.haystack.includes(needle) || entry.row.title.toLowerCase().includes(needle))
        .map((entry): WalletRow => entry.row),
    }
  }).filter((group) => group.rows.length > 0)
})

const captionText = computed(() =>
  typeof props.caption === 'function' ? (current.value ? props.caption(current.value.balance) : '') : (props.caption ?? ''),
)
const label = computed(() => {
  const name = props.variant === 'field' ? current.value?.title : current.value?.fullName
  return name ?? props.emptyLabel ?? props.placeholder ?? t('quickAdd.pickStore')
})
/** One trigger for every look, so the field is never swapped for another control (rule 17). */
const TRIGGER_CLASS: Record<'field' | 'plate' | 'inline', string> = {
  field: 'w-full justify-between gap-2 px-3 font-normal',
  plate: 'h-13 w-full justify-start gap-3 rounded-xl bg-muted px-3.5 text-left font-normal hover:bg-muted/80',
  inline: 'h-auto min-w-0 justify-start gap-3 px-1 py-1 text-left font-normal',
}
const INVALID_CLASS = { field: 'border-destructive', plate: 'ring-1 ring-destructive', inline: 'text-destructive' }
const triggerClass = computed(() =>
  cn(
    TRIGGER_CLASS[props.variant],
    !current.value && 'text-muted-foreground',
    props.invalid && INVALID_CLASS[props.variant],
    attrs.class as string,
  ),
)

function pick(id: string) {
  model.value = id
  open.value = false
  search.value = ''
}
</script>

<template>
  <PickerSheet
    v-model:open="open"
    v-model:search="search"
    overlay="store-select"
    :title="props.placeholder ?? t('quickAdd.store')"
    :search-label="t('quickAdd.searchStore')"
  >
    <template #trigger>
      <Button
        type="button"
        :variant="props.variant === 'field' ? 'outline' : 'ghost'"
        v-bind="attrs"
        :data-wallet-plate="props.variant === 'plate' || undefined"
        :data-wallet-inline="props.variant === 'inline' || undefined"
        :aria-invalid="props.invalid || undefined"
        :class="triggerClass"
      >
        <component :is="current.icon" v-if="current && props.variant !== 'field'" class="size-5 shrink-0 text-muted-foreground" />
        <span v-if="props.variant === 'inline'" class="grid min-w-0">
          <span class="truncate text-sm font-medium">{{ label }}</span>
          <span v-if="captionText" class="truncate text-xs text-muted-foreground">{{ captionText }}</span>
        </span>
        <span v-else class="min-w-0 flex-1 truncate" :class="props.variant === 'plate' && current && 'text-base font-medium'">
          {{ label }}
        </span>
        <span v-if="current && props.variant === 'plate'" class="shrink-0 text-sm text-muted-foreground tabular-nums">
          {{ current.balance }}
        </span>
        <ChevronDown v-if="props.variant !== 'inline'" class="size-4 shrink-0 text-muted-foreground" />
      </Button>
    </template>
    <PickerRow v-if="props.emptyLabel" :title="props.emptyLabel" :selected="model === ''" @click="pick('')" />
    <p v-if="groups.length === 0" class="py-6 text-center text-sm text-muted-foreground">{{ t('quickAdd.noStore') }}</p>
    <!-- money: plain — each group total and row balance is preformatted in `groups` / `walletRow` -->
    <PickerSection v-for="group in groups" :key="group.purpose" :label="t(`purpose.${group.purpose}`)" :total="group.total">
      <PickerRow
        v-for="row in group.rows"
        :key="row.id"
        :title="row.fullName"
        :subtitle="row.subtitle"
        :amount="row.balance"
        :amount-hint="row.baseHint"
        :selected="row.id === model"
        @click="pick(row.id)"
      >
        <template #icon><component :is="row.icon" :style="row.color ? { color: row.color } : undefined" /></template>
      </PickerRow>
    </PickerSection>
  </PickerSheet>
</template>
