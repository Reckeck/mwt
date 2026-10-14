<script setup lang="ts">
import { computed } from 'vue'
import { type RouteLocationRaw, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { useEventListener } from '@vueuse/core'
import { Lock, Plus, RefreshCw, Settings, Sparkles } from 'lucide-vue-next'

import { usePriceRefresh } from '@/features/price-refresh'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from '@/shared/ui/command'
import { Dialog } from '@/shared/ui/dialog'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { storeIcon } from '@/shared/lib/store-icons'

import { NAV_GROUPS } from '../../app-shell.constants'

/**
 * Ctrl+K: go to any screen or wallet, or run a main action, by typing a few
 * letters. The same list is reachable without a mouse from anywhere.
 */
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ quickAdd: []; settings: []; lock: [] }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const priceRefresh = usePriceRefresh()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const wallets = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))
const screens = computed(() => NAV_GROUPS.flatMap((group) => group.items))

useEventListener(window, 'keydown', (event: KeyboardEvent) => {
  if ((event.ctrlKey || event.metaKey) && event.code === 'KeyK') {
    event.preventDefault()
    open.value = !open.value
  }
})

function run(action: () => void) {
  open.value = false
  action()
}
const go = (to: RouteLocationRaw) => run(() => void router.push(to))
</script>

<template>
  <Dialog v-model:open="open" :title="t('palette.title')" :description="t('palette.hint')" size="md">
    <Command class="rounded-md border border-border">
      <CommandInput :placeholder="t('palette.placeholder')" />
      <CommandList class="max-h-80">
        <CommandEmpty>{{ t('palette.nothing') }}</CommandEmpty>
        <CommandGroup :heading="t('palette.actions')">
          <CommandItem value="action-quick-add" @select="run(() => emit('quickAdd'))">
            <Plus />
            {{ t('quickAdd.title') }}
          </CommandItem>
          <CommandItem value="action-refresh" @select="run(() => priceRefresh.mutate())">
            <RefreshCw />
            {{ t('store.refresh') }}
          </CommandItem>
          <CommandItem value="action-settings" @select="run(() => emit('settings'))">
            <Settings />
            {{ t('settingsShell.title') }}
          </CommandItem>
          <CommandItem value="action-whats-new" @select="go({ query: { ...route.query, whatsNew: '1' } })">
            <Sparkles />
            {{ t('whatsNew.title') }}
          </CommandItem>
          <CommandItem value="action-lock" @select="run(() => emit('lock'))">
            <Lock />
            {{ t('auth.lock') }}
          </CommandItem>
        </CommandGroup>
        <CommandSeparator />
        <CommandGroup :heading="t('palette.screens')">
          <CommandItem v-for="item in screens" :key="item.name" :value="`screen-${item.name}`" @select="go({ name: item.name })">
            <component :is="item.icon" />
            {{ t(item.key) }}
          </CommandItem>
        </CommandGroup>
        <CommandSeparator />
        <CommandGroup :heading="t('palette.wallets')">
          <CommandItem
            v-for="node in wallets"
            :key="node.store.id"
            :value="`wallet-${node.store.id}`"
            @select="go({ name: 'wallet', params: { id: node.store.id } })"
          >
            <component :is="storeIcon(node.store)" :style="{ color: node.store.color }" />
            {{ node.path.join(' › ') }}
            <CommandShortcut v-if="node.depth === 0">{{ t(`stores.kind.${node.store.kind}`) }}</CommandShortcut>
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </Command>
  </Dialog>
</template>
