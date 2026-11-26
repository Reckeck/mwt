<script setup lang="ts">
import { computed, ref, useAttrs } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { decimalAdd } from '@/shared/lib/decimal'
import { formatMoney } from '@/shared/lib/format'
import { useSettingsStore } from '@/shared/store/settings'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { PickerSheet } from '@/shared/ui/picker-sheet'

import { walletRow, type WalletRow } from '../../lib/wallet-option'

/** The line under the name of an `inline` trigger; a function gets the picked wallet's balance (`отдал · на счёте 52 000 ₽`). */
type StoreCaption = string | ((balance: string) => string)

/**
 * A wallet picked from a bottom sheet on a phone (board 532847), a popover under the trigger on a
 * desktop (`PickerSheet`): a search field, then the wallets grouped by
 * what the money is for, each group with its total, each row an icon circle, the name, the bound
 * asset under it and the balance on the right. The trigger is an outline field (`field`), the muted
 * wallet plate of the quick form (`plate`, board 533927), or the bare wallet line inside an exchange
 * box with a caption under the name (`inline`, board 534325). `id`,
 * `aria-*` and `class` go to the trigger.
 */
defineOptions({ inheritAttrs: false })
const props = withDefaults(
  defineProps<{ nodes: StoreTreeNode[]; invalid?: boolean; placeholder?: string; variant?: 'field' | 'plate' | 'inline'; caption?: StoreCaption }>(),
  { variant: 'field', placeholder: undefined, caption: undefined },
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
  return name ?? props.placeholder ?? t('quickAdd.pickStore')
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
    <p v-if="groups.length === 0" class="py-6 text-center text-sm text-muted-foreground">{{ t('quickAdd.noStore') }}</p>
    <!-- money: plain — each group's total in its heading is preformatted in `groups` -->
    <section v-for="group in groups" :key="group.purpose" class="grid gap-1 pb-2">
      <h3 class="flex justify-between gap-2 px-1 pt-2 text-xs font-medium text-muted-foreground">
        <span>{{ t(`purpose.${group.purpose}`) }}</span>
        <span class="tabular-nums">{{ group.total }}</span>
      </h3>
      <Button
        v-for="row in group.rows"
        :key="row.id"
        type="button"
        variant="ghost"
        class="h-auto justify-start gap-3 px-1 py-2 text-left"
        :aria-pressed="row.id === model"
        :class="row.id === model && 'bg-accent'"
        @click="pick(row.id)"
      >
        <span class="grid size-10 shrink-0 place-items-center rounded-full bg-muted [&_svg]:size-5">
          <component :is="row.icon" :style="row.color ? { color: row.color } : undefined" />
        </span>
        <span class="grid min-w-0 flex-1">
          <span class="truncate text-base font-medium">{{ row.fullName }}</span>
          <span v-if="row.subtitle" class="truncate text-xs font-normal text-muted-foreground">{{ row.subtitle }}</span>
        </span>
        <span class="shrink-0 text-sm font-semibold tabular-nums">{{ row.balance }}</span>
      </Button>
    </section>
  </PickerSheet>
</template>
