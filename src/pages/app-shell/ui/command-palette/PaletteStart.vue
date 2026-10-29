<script setup lang="ts">
import type { Component } from 'vue'
import { useI18n } from 'vue-i18n'
import { type RouteLocationRaw, useRoute, useRouter } from 'vue-router'
import { ArrowLeftRight, ArrowRight, History, Plus, WalletCards } from 'lucide-vue-next'

import { CommandGroup, CommandItem, CommandShortcut } from '@/shared/ui/command'
import { ENTITY_ICONS } from '@/shared/ui/entity-link'
import { entityKey, entityLocation } from '@/shared/lib/entity-link'
import { SECTION_HOTKEYS, hotkeyLabel } from '@/shared/lib/hotkeys'
import { openQuickAdd } from '@/shared/lib/quick-add'

import { NAV_ITEMS } from '../../app-shell.constants'
import type { RecentRow } from './use-palette-recent'

/**
 * An empty Ctrl K (Paths 1 · 7): `Recent` — where one was a minute ago,
 * `Go to` with Alt 1…5, `Record` with Ctrl N.
 */
const props = defineProps<{ recent: RecentRow[]; run: (action: () => void) => void }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const go = (to: RouteLocationRaw) => props.run(() => void router.push(to))

const RECORD: { kind: TxKind; icon: Component; key: string; hotkey?: boolean }[] = [
  { kind: 'payment', icon: Plus, key: 'palette.record.payment', hotkey: true },
  { kind: 'convert', icon: ArrowLeftRight, key: 'palette.record.convert' },
  { kind: 'transfer', icon: ArrowRight, key: 'palette.record.transfer' },
]

/** A screen shows its section's icon; a card its kind's. */
function iconOf(row: RecentRow): Component {
  if (row.entity) return ENTITY_ICONS[row.entity.kind] ?? History
  if (row.wallet) return WalletCards
  return NAV_ITEMS.find((item) => item.prefix && row.path?.startsWith(item.prefix))?.icon ?? History
}

/** A wallet screen glances like the wallet itself. */
function keyOf(row: RecentRow): string | undefined {
  if (row.entity) return entityKey(row.entity)
  const id = row.wallet ? row.path?.split('/')[2] : undefined
  return id ? entityKey({ kind: 'store', id }) : undefined
}

function openRow(row: RecentRow) {
  if (row.entity) go(entityLocation(row.entity, route.query))
  else if (row.path) go(row.path)
}
</script>

<template>
  <CommandGroup v-if="props.recent.length" :heading="t('palette.recent')" data-palette-recent>
    <CommandItem
      v-for="row in props.recent"
      :key="row.id"
      :value="row.id"
      :data-entity="keyOf(row)"
      :data-title="row.title"
      @select="openRow(row)"
    >
      <component :is="iconOf(row)" />
      <span class="min-w-0 flex-1 truncate">{{ row.title }}</span>
      <span class="shrink-0 text-muted-foreground">{{ row.note }}</span>
    </CommandItem>
  </CommandGroup>
  <CommandGroup :heading="t('palette.go')" data-palette-go>
    <CommandItem v-for="(item, index) in NAV_ITEMS" :key="item.name" :value="`screen-${item.name}`" @select="go({ name: item.name })">
      <component :is="item.icon" />
      <span class="min-w-0 flex-1 truncate">{{ t(item.key) }}</span>
      <CommandShortcut v-if="SECTION_HOTKEYS[index]" class="tracking-normal">{{ hotkeyLabel(SECTION_HOTKEYS[index]) }}</CommandShortcut>
    </CommandItem>
  </CommandGroup>
  <CommandGroup :heading="t('palette.record')" data-palette-record>
    <CommandItem v-for="entry in RECORD" :key="entry.kind" :value="`record-${entry.kind}`" @select="props.run(() => openQuickAdd({ kind: entry.kind }))">
      <component :is="entry.icon" />
      <span class="min-w-0 flex-1 truncate">{{ t(entry.key) }}</span>
      <CommandShortcut v-if="entry.hotkey" class="tracking-normal">{{ hotkeyLabel('quickAdd') }}</CommandShortcut>
    </CommandItem>
  </CommandGroup>
</template>
