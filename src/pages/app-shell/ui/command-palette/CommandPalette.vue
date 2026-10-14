<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { type RouteLocationRaw, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { onClickOutside, useEventListener } from '@vueuse/core'
import { Lock, Plus, RefreshCw, Settings, Sparkles } from 'lucide-vue-next'

import { walletRow } from '@/entities/store'
import { usePriceRefresh } from '@/features/price-refresh'
import { Command, CommandEmpty, CommandGroup, CommandItem, CommandList, CommandSeparator } from '@/shared/ui/command'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

import { NAV_GROUPS } from '../../app-shell.constants'
import PaletteInput from './PaletteInput.vue'

/**
 * The search in the header: type a few letters to go to any screen or wallet,
 * or run a main action. The list drops down under the field — no window over
 * the page. Ctrl+K puts the cursor in it from anywhere, Esc closes it.
 */
const emit = defineEmits<{ quickAdd: []; settings: []; lock: [] }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const settings = useSettingsStore()
const priceRefresh = usePriceRefresh()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const wallets = computed(() =>
  flattenStoreTree(tree.data.value ?? [])
    .filter((node) => !node.store.archived)
    .map((node) => walletRow(node, t, settings.baseCurrency)),
)
const screens = computed(() => NAV_GROUPS.flatMap((group) => group.items))

const open = ref(false)
const root = ref<HTMLElement | null>(null)
const input = ref<InstanceType<typeof PaletteInput> | null>(null)
/** The listbox keeps the last pick; cleared so the same row can be picked again. */
const picked = ref<string>('')

onClickOutside(root, () => (open.value = false))
useEventListener(root, 'keydown', (event: KeyboardEvent) => {
  if (event.key === 'Escape') close()
})
useEventListener(window, 'keydown', (event: KeyboardEvent) => {
  if ((event.ctrlKey || event.metaKey) && event.code === 'KeyK') {
    event.preventDefault()
    open.value = true
    void nextTick(() => input.value?.focus())
  }
})

function close() {
  open.value = false
  input.value?.blur()
}

function run(action: () => void) {
  close()
  picked.value = ''
  action()
}
const go = (to: RouteLocationRaw) => run(() => void router.push(to))
</script>

<template>
  <div ref="root" class="relative w-40 min-w-0 sm:w-56 lg:w-72" data-tour="command-palette">
    <Command v-model="picked" class="overflow-visible bg-transparent">
      <PaletteInput ref="input" :placeholder="t('palette.placeholder')" :label="t('palette.title')" @focus="open = true" />
      <div
        v-show="open"
        class="absolute top-full right-0 z-50 mt-1 w-[min(26rem,calc(100vw-2rem))] overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-md"
      >
        <CommandList class="max-h-[min(24rem,70vh)]">
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
            <!-- The same two lines as «To» in an operation: the name, then parent or kind · asset · balance. -->
            <CommandItem
              v-for="row in wallets"
              :key="row.id"
              :value="`wallet-${row.id}`"
              @select="go({ name: 'wallet', params: { id: row.id } })"
            >
              <component :is="row.icon" :style="row.color ? { color: row.color } : undefined" />
              <span class="grid min-w-0 leading-tight">
                <span class="truncate font-medium">{{ row.title }}</span>
                <span class="truncate text-xs text-muted-foreground">{{ row.detail }}</span>
              </span>
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </div>
    </Command>
  </div>
</template>
