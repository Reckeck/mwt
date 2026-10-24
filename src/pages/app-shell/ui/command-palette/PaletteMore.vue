<script setup lang="ts">
import { computed } from 'vue'
import { type RouteLocationRaw, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowLeftRight, Lock, Plus, RefreshCw, Settings, Sparkles } from 'lucide-vue-next'

import { walletRow } from '@/entities/store'
import { usePriceRefresh } from '@/features/price-refresh'
import { useStoreTree } from '@/shared/api/queries'
import { CommandGroup, CommandItem, CommandShortcut } from '@/shared/ui/command'
import { openConverter } from '@/shared/lib/converter'
import { entityKey } from '@/shared/lib/entity-link'
import { hotkeyLabel } from '@/shared/lib/hotkeys'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'
import { useVaultStore } from '@/shared/store/vault'

import { NAV_ITEMS } from '../../app-shell.constants'
import { usePaletteIndex } from './use-palette-index'

/**
 * What a typed query also finds (Shell · 4): the app's actions, the wallets,
 * settings and sub-tabs. An empty Ctrl K keeps to `PaletteStart`.
 */
const props = defineProps<{ run: (action: () => void) => void }>()
const emit = defineEmits<{ quickAdd: []; settings: []; lock: [] }>()
const vault = useVaultStore()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const settings = useSettingsStore()
const priceRefresh = usePriceRefresh()
const go = (to: RouteLocationRaw) => props.run(() => void router.push(to))

const tree = useStoreTree()
const wallets = computed(() =>
  flattenStoreTree(tree.data.value ?? [])
    .filter((node) => !node.store.archived)
    .map((node) => walletRow(node, t, settings.baseCurrency)),
)
const index = usePaletteIndex(computed(() => NAV_ITEMS.map((item) => item.name)))
/** Settings and sub-tabs: a heading and its entries each. */
const groups = computed(() => [
  { key: 'settings', heading: t('palette.settings'), items: index.settings.value },
  { key: 'tabs', heading: t('palette.tabs'), items: index.tabs.value },
])
</script>

<template>
  <CommandGroup :heading="t('palette.actions')">
    <CommandItem value="action-quick-add" @select="props.run(() => emit('quickAdd'))">
      <Plus />
      {{ t('quickAdd.title') }}
      <CommandShortcut>{{ hotkeyLabel('quickAdd') }}</CommandShortcut>
    </CommandItem>
    <CommandItem value="action-refresh" @select="props.run(() => priceRefresh.mutate())">
      <RefreshCw />
      {{ t('store.refresh') }}
    </CommandItem>
    <CommandItem value="action-settings" @select="props.run(() => emit('settings'))">
      <Settings />
      {{ t('settingsShell.title') }}
    </CommandItem>
    <CommandItem value="action-whats-new" @select="go({ query: { ...route.query, whatsNew: '1' } })">
      <Sparkles />
      {{ t('whatsNew.title') }}
    </CommandItem>
    <CommandItem value="action-converter" @select="props.run(() => openConverter())">
      <ArrowLeftRight />
      {{ t('palette.converter') }}
    </CommandItem>
    <CommandItem v-if="vault.encrypted" value="action-lock" @select="props.run(() => emit('lock'))">
      <Lock />
      {{ t('auth.lock') }}
    </CommandItem>
  </CommandGroup>
  <CommandGroup :heading="t('palette.wallets')">
    <!-- The same two lines as «To» in an operation: the name, then parent or kind · asset · balance. -->
    <CommandItem
      v-for="row in wallets"
      :key="row.id"
      :value="`wallet-${row.id}`"
      :data-entity="entityKey({ kind: 'store', id: row.id })"
      :data-title="row.title"
      @select="go({ name: 'wallet', params: { id: row.id } })"
    >
      <component :is="row.icon" :style="row.color ? { color: row.color } : undefined" />
      <span class="grid min-w-0 leading-tight">
        <span class="truncate font-medium">{{ row.title }}</span>
        <span class="truncate text-xs text-muted-foreground">{{ row.detail }}</span>
      </span>
    </CommandItem>
  </CommandGroup>
  <template v-for="group in groups" :key="group.key">
    <template v-if="group.items.length">
      <CommandGroup :heading="group.heading">
        <CommandItem v-for="entry in group.items" :key="entry.id" :value="entry.id" @select="go(entry.to)">
          {{ entry.label }}
          <span class="sr-only">{{ entry.keywords }}</span>
        </CommandItem>
      </CommandGroup>
    </template>
  </template>
</template>
