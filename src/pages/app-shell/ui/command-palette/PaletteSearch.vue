<script setup lang="ts">
import { computed, toRef, watch } from 'vue'
import { type RouteLocationRaw, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useEventListener } from '@vueuse/core'
import { ArrowLeft, ArrowRightLeft, ExternalLink, Plus, ShoppingCart, Target } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { CommandEmpty, CommandGroup, CommandItem, CommandShortcut, useCommand } from '@/shared/ui/command'
import { ENTITY_ICONS, useEntityActions } from '@/shared/ui/entity-link'
import { spendingOfCategory } from '@/shared/lib/entity-actions'
import { entityKey, entityLocation, parseEntityKey } from '@/shared/lib/entity-link'
import { formatDayMonthShort, formatMoney, formatMonthKey } from '@/shared/lib/format'
import { hotkeyLabel } from '@/shared/lib/hotkeys'
import { openQuickAdd } from '@/shared/lib/quick-add'
import { txId, txKind, txMeta, txPrincipalLegs } from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'

import PaletteStart from './PaletteStart.vue'
import { usePaletteRecent } from './use-palette-recent'
import { usePaletteSearch } from './use-palette-search'

export interface PaletteTarget {
  target: EntityRef
  title: string
}

/**
 * The vault part of the palette: entities by name, operations by text or
 * amount and, for a category the query names, what to do with it (Paths 2 · 8).
 * → on an entity lists its actions, ← goes back.
 */
const props = defineProps<{ open: boolean; run: (action: () => void) => void }>()
const actionsFor = defineModel<PaletteTarget | null>('actionsFor', { required: true })

const { t } = useI18n()
const route = useRoute()
const context = useTxContext()
const actionsOf = useEntityActions()
const { filterState } = useCommand()
const search = computed(() => filterState.search)
const found = usePaletteSearch(toRef(props, 'open'), search)
const recent = usePaletteRecent(found.recent)

const router = useRouter()
const go = (to: RouteLocationRaw) => props.run(() => void router.push(to))

/** A category opens in «Траты» on this month; anything else on its card. */
function openTarget(target: EntityRef) {
  if (target.kind === 'category') {
    const month = found.monthTotals.value.month
    go(spendingOfCategory(target.id, month))
  } else go(entityLocation(target, route.query))
}
const actions = computed(() =>
  actionsFor.value ? actionsOf(actionsFor.value.target, actionsFor.value.title).filter((action) => !action.hidden) : [],
)

/** «36 $ в окт.» beside a category. */
function monthNote(id: string): string {
  const { totals, currency, month } = found.monthTotals.value
  const total = totals.get(id)
  if (!total) return ''
  return t('palette.inMonth', { sum: formatMoney(total, { decimals: 0, currency }), month: formatMonthKey(month, { month: 'short' }) })
}

function txRow(tx: Transaction) {
  const meta = txMeta(tx)
  const leg = txPrincipalLegs(tx)[0]
  const category = meta.categories[0]
  return {
    id: txId(tx),
    payment: txKind(tx) === 'payment',
    label: meta.comment || (category ? context.categoryName(category) : t('entity.kind.tx')),
    day: formatDayMonthShort(meta.timestamp),
    amount: leg?.amount ?? null,
    currency: leg?.asset.symbol.toLowerCase(),
  }
}
/** A few, as on the frame: the rest are one «Операции» search away. */
const transactions = computed(() => found.transactions.value.slice(0, 3).map(txRow))

const category = computed(() => found.category.value)
function recordInto(id: string) {
  props.run(() => openQuickAdd({ kind: 'payment', category: id }))
}
function limitOn(id: string) {
  go({ name: 'money-plan', query: { new: 'limit', category: id } })
}

watch(actionsFor, () => {
  filterState.search = ''
})
useEventListener(window, 'keydown', (event: KeyboardEvent) => {
  if (!props.open) return
  if (actionsFor.value) {
    if (event.key === 'ArrowLeft' || (event.key === 'Backspace' && !filterState.search)) {
      event.preventDefault()
      actionsFor.value = null
    }
    return
  }
  const row = document.querySelector<HTMLElement>('[data-command-palette] [data-highlighted][data-entity]')
  const target = row?.dataset.entity ? parseEntityKey(row.dataset.entity) : null
  // Ctrl Enter: a payment into the highlighted category, or the one the query names.
  if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
    const id = target?.kind === 'category' ? target.id : category.value?.target.id
    if (!id) return
    event.preventDefault()
    event.stopPropagation()
    recordInto(id)
    return
  }
  if (event.key !== 'ArrowRight' || !target || !row) return
  event.preventDefault()
  actionsFor.value = { target, title: row.dataset.title ?? '' }
})
</script>

<template>
  <template v-if="actionsFor">
    <CommandGroup :heading="actionsFor.title || t(`entity.kind.${actionsFor.target.kind}`)">
      <CommandItem value="entity-actions-back" @select="actionsFor = null">
        <ArrowLeft />
        {{ t('palette.back') }}
      </CommandItem>
      <CommandItem value="entity-actions-open" @select="go(entityLocation(actionsFor.target, route.query))">
        <ExternalLink />
        {{ t('palette.open') }}
      </CommandItem>
      <CommandItem
        v-for="action in actions"
        :key="action.id"
        :value="`entity-action-${action.id}`"
        :disabled="Boolean(action.disabled)"
        @select="props.run(() => action.run?.())"
      >
        <component :is="action.icon" v-if="action.icon" />
        {{ action.label }}
      </CommandItem>
    </CommandGroup>
  </template>

  <template v-else-if="search">
    <CommandEmpty v-if="!transactions.length">{{ t('palette.nothing') }}</CommandEmpty>

    <!-- Entities are listed only while searching: a full list would bury the screens and wallets. -->
    <CommandGroup v-for="group in found.groups.value" :key="group.kind" :heading="t(`palette.group.${group.kind}`)">
      <CommandItem
        v-for="hit in group.items"
        :key="hit.id"
        :value="hit.id"
        :data-entity="entityKey(hit.target)"
        :data-title="hit.label"
        :class="hit.archived && 'opacity-60'"
        @select="openTarget(hit.target)"
      >
        <component :is="ENTITY_ICONS[hit.target.kind]" />
        <span class="min-w-0 flex-1 truncate">{{ hit.label }}</span>
        <span v-if="hit.target.kind === 'category'" class="shrink-0 text-muted-foreground tabular-nums">{{ monthNote(hit.target.id) }}</span>
        <span class="sr-only">{{ t(`entity.kind.${hit.target.kind}`) }}</span>
      </CommandItem>
    </CommandGroup>

    <CommandGroup v-if="transactions.length" :heading="t('palette.transactions')">
      <!-- Matched by amount or wallet too: the row carries the typed text so the list filter keeps it. -->
      <CommandItem
        v-for="row in transactions"
        :key="`${search}|${row.id}`"
        :value="`tx-${row.id}`"
        :data-entity="entityKey({ kind: 'tx', id: row.id })"
        :data-title="row.label"
        @select="openTarget({ kind: 'tx', id: row.id })"
      >
        <ShoppingCart v-if="row.payment" />
        <ArrowRightLeft v-else />
        <span class="min-w-0 flex-1 truncate">{{ row.label }} · {{ row.day }}</span>
        <Money v-if="row.amount" class="shrink-0 text-muted-foreground" :amount="row.amount" :currency="row.currency" :convert="false" />
        <span class="sr-only">{{ search }}</span>
      </CommandItem>
    </CommandGroup>

    <CommandGroup v-if="category" :heading="t('palette.actions')" data-palette-category-actions>
      <CommandItem :value="`category-record-${category.target.id}`" @select="recordInto(category.target.id)">
        <Plus />
        <span class="min-w-0 flex-1 truncate">{{ t('palette.recordInto', { name: category.label }) }}</span>
        <CommandShortcut class="tracking-normal">{{ hotkeyLabel('saveAndNext') }}</CommandShortcut>
        <span class="sr-only">{{ search }}</span>
      </CommandItem>
      <CommandItem :value="`category-limit-${category.target.id}`" @select="limitOn(category.target.id)">
        <Target />
        <span class="min-w-0 flex-1 truncate">{{ t('palette.limitOn', { name: category.label }) }}</span>
        <span class="sr-only">{{ search }}</span>
      </CommandItem>
    </CommandGroup>
  </template>

  <PaletteStart v-else :recent="recent" :run="props.run" />
</template>
