<script setup lang="ts">
import { computed, toRef, watch } from 'vue'
import { type RouteLocationRaw, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useEventListener } from '@vueuse/core'
import { ArrowLeft, ArrowRightLeft, ExternalLink, History } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { CommandEmpty, CommandGroup, CommandItem, CommandSeparator, useCommand } from '@/shared/ui/command'
import { ENTITY_ICONS, useEntityActions } from '@/shared/ui/entity-link'
import { entityKey, entityLocation, parseEntityKey } from '@/shared/lib/entity-link'
import { formatDay } from '@/shared/lib/format'
import { txId, txMeta, txPrincipalLegs } from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'

import { usePaletteSearch } from './use-palette-search'

export interface PaletteTarget {
  target: EntityRef
  title: string
}

/**
 * The vault part of the palette: entities by name, operations by text or
 * amount, the cards opened lately; → on an entity lists its actions, ← goes back.
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

const router = useRouter()
const go = (to: RouteLocationRaw) => props.run(() => void router.push(to))

const openTarget = (target: EntityRef) => go(entityLocation(target, route.query))
const actions = computed(() =>
  actionsFor.value ? actionsOf(actionsFor.value.target, actionsFor.value.title).filter((action) => !action.hidden) : [],
)

function txRow(tx: Transaction) {
  const meta = txMeta(tx)
  const leg = txPrincipalLegs(tx)[0]
  const category = meta.categories[0]
  return {
    id: txId(tx),
    label: meta.comment || (category ? context.categoryName(category) : t('entity.kind.tx')),
    day: formatDay(meta.timestamp),
    amount: leg?.amount ?? null,
    currency: leg?.asset.symbol.toLowerCase(),
  }
}
const transactions = computed(() => found.transactions.value.map(txRow))

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
  if (event.key !== 'ArrowRight') return
  const row = document.querySelector<HTMLElement>('[data-command-palette] [data-highlighted][data-entity]')
  const target = row?.dataset.entity ? parseEntityKey(row.dataset.entity) : null
  if (!target || !row) return
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
      <CommandItem value="entity-actions-open" @select="openTarget(actionsFor.target)">
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

  <template v-else>
    <CommandEmpty v-if="!transactions.length">{{ t('palette.nothing') }}</CommandEmpty>

    <CommandGroup v-if="!search && found.recent.value.length" :heading="t('palette.recent')">
      <CommandItem
        v-for="hit in found.recent.value"
        :key="hit.id"
        :value="hit.id"
        :data-entity="entityKey(hit.target)"
        :data-title="hit.label"
        @select="openTarget(hit.target)"
      >
        <History />
        <span class="truncate">{{ hit.label }}</span>
        <span class="ml-auto shrink-0 text-xs text-muted-foreground">{{ t(`entity.kind.${hit.target.kind}`) }}</span>
      </CommandItem>
    </CommandGroup>

    <!-- Entities are listed only while searching: a full list would bury the screens and wallets. -->
    <template v-if="search">
      <template v-for="group in found.groups.value" :key="group.kind">
        <CommandSeparator />
        <CommandGroup :heading="t(`palette.group.${group.kind}`)">
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
            <span class="truncate">{{ hit.label }}</span>
            <span class="sr-only">{{ t(`entity.kind.${hit.target.kind}`) }}</span>
          </CommandItem>
        </CommandGroup>
      </template>
    </template>

    <template v-if="transactions.length">
      <CommandSeparator />
      <CommandGroup :heading="t('palette.transactions')">
        <!-- Matched by amount or wallet too: the row carries the typed text so the list filter keeps it. -->
        <CommandItem
          v-for="row in transactions"
          :key="`${search}|${row.id}`"
          :value="`tx-${row.id}`"
          :data-entity="entityKey({ kind: 'tx', id: row.id })"
          :data-title="row.label"
          @select="openTarget({ kind: 'tx', id: row.id })"
        >
          <ArrowRightLeft />
          <span class="grid min-w-0 leading-tight">
            <span class="truncate">{{ row.label }}</span>
            <span class="truncate text-xs text-muted-foreground">{{ row.day }}</span>
          </span>
          <Money v-if="row.amount" class="ml-auto shrink-0 text-xs" :amount="row.amount" :currency="row.currency" :convert="false" />
          <span class="sr-only">{{ search }}</span>
        </CommandItem>
      </CommandGroup>
    </template>
  </template>
</template>
