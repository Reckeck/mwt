<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import {
  ChartArea,
  FlaskConical,
  LayoutDashboard,
  Lock,
  Menu,
  ScrollText,
  Settings,
  Wallet,
} from 'lucide-vue-next'

import Button from '@/components/ui/button.vue'
import SidebarWallets from '@/components/SidebarWallets.vue'
import { call } from '@/lib/ipc'
import { cn } from '@/lib/utils'
import { useSettingsStore } from '@/stores/settings'
import { useVaultStore } from '@/stores/vault'
import { useTheme } from '@/theme'
import { sidebarState, useViewport } from '@/lib/viewport'

const { t } = useI18n()
const route = useRoute()
const vault = useVaultStore()
const settingsStore = useSettingsStore()

useTheme()

const width = useViewport()
const sidebar = computed(() => sidebarState(width.value))
const showLabels = computed(() => sidebar.value !== 'rail')
const menuOpen = ref(false)

// deduped with the identical query inside <SidebarWallets />
const storesQuery = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const navItems = [
  { name: 'dashboard', icon: LayoutDashboard, key: 'nav.dashboard' },
  { name: 'stores', icon: Wallet, key: 'nav.stores' },
  { name: 'transactions', icon: ScrollText, key: 'nav.transactions' },
  { name: 'charts', icon: ChartArea, key: 'nav.charts' },
  { name: 'strategy-lab', icon: FlaskConical, key: 'nav.strategyLab' },
  { name: 'settings', icon: Settings, key: 'nav.settings' },
]

const titleKey = computed(() => (route.meta.i18n as string | undefined) ?? 'app.title')

/** On the wallet page the store name is a more useful title than the nav label. */
const walletTitle = computed(() => {
  if (route.name !== 'wallet') return ''
  const id = String(route.params.id ?? '')
  return storesQuery.data.value?.find((s) => s.id === id)?.name ?? t('nav.wallets')
})

const pageTitle = computed(() => (route.name === 'wallet' ? walletTitle.value : t(titleKey.value)))

const asideClass = computed(() => {
  if (sidebar.value === 'compact') {
    return cn(
      'fixed inset-y-0 left-0 z-40 flex w-56 flex-col border-r border-border bg-card transition-transform',
      menuOpen.value ? 'translate-x-0' : '-translate-x-full',
    )
  }
  return cn(
    'flex shrink-0 flex-col border-r border-border bg-card',
    sidebar.value === 'rail' ? 'w-16' : 'w-56',
  )
})

async function lock() {
  await vault.lock()
}
</script>

<template>
  <div class="flex h-full">
    <aside :class="asideClass">
      <div
        class="flex h-14 shrink-0 items-center gap-2 border-b border-border"
        :class="showLabels ? 'px-4' : 'justify-center px-0'"
      >
        <div class="flex size-7 shrink-0 items-center justify-center rounded-md bg-primary text-sm font-bold text-primary-foreground">
          T
        </div>
        <span v-if="showLabels" class="truncate text-sm font-semibold">{{ t('app.title') }}</span>
      </div>

      <nav class="space-y-1 p-3 pb-1">
        <RouterLink
          v-for="item in navItems"
          :key="item.name"
          :to="{ name: item.name }"
          :title="sidebar === 'rail' ? t(item.key) : undefined"
          class="flex items-center gap-2.5 rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
          :class="cn(route.name === item.name && 'bg-accent text-accent-foreground font-medium', sidebar === 'rail' && 'justify-center px-0')"
        >
          <component :is="item.icon" class="size-4 shrink-0" />
          <span v-if="showLabels" class="truncate">{{ t(item.key) }}</span>
        </RouterLink>
      </nav>

      <SidebarWallets :show-labels="showLabels" />

      <div class="border-t border-border p-3">
        <div v-if="showLabels" class="mb-2 truncate text-xs text-muted-foreground">{{ vault.status?.path }}</div>
        <Button
          variant="outline"
          size="sm"
          class="w-full"
          :class="sidebar === 'rail' && 'aspect-square w-full p-0'"
          @click="lock"
        >
          <Lock class="size-4 shrink-0" />
          <span v-if="showLabels">{{ t('auth.lock') }}</span>
        </Button>
      </div>
    </aside>

    <div
      v-if="sidebar === 'compact' && menuOpen"
      class="fixed inset-0 z-30 bg-black/40"
      @click="menuOpen = false"
    />

    <main class="flex min-w-0 flex-1 flex-col">
      <header class="flex h-14 shrink-0 items-center gap-2 border-b border-border px-4 md:px-6">
        <button
          v-if="sidebar === 'compact'"
          type="button"
          class="flex size-8 shrink-0 items-center justify-center rounded-md hover:bg-accent"
          @click="menuOpen = true"
        >
          <span class="sr-only">{{ t('nav.dashboard') }}</span>
          <Menu class="size-5" />
        </button>
        <h1 class="min-w-0 flex-1 truncate text-base font-semibold">{{ pageTitle }}</h1>
        <div v-if="sidebar !== 'compact'" class="flex shrink-0 items-center gap-2 text-sm text-muted-foreground">
          <span v-if="settingsStore.settings">
            {{ settingsStore.settings.base_currency.toUpperCase() }}
            · {{ t('settings.theme.' + settingsStore.settings.theme) }}
          </span>
        </div>
      </header>

      <div class="min-h-0 flex-1 overflow-y-auto overflow-x-auto p-4 md:p-6 @container">
        <RouterView />
      </div>
    </main>
  </div>
</template>