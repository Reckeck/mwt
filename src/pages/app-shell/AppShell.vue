<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

import {
  CandlestickChart,
  ChartArea,
  LayoutDashboard,
  Lock,
  Menu,
  ScrollText,
  Settings,
  WifiOff,
} from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { AppSettingsModal } from '@/widgets/app-settings'
import { SidebarWallets } from '@/widgets/sidebar-wallets'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'
import { call, onEvent } from '@/shared/api/ipc'
import { cn } from '@/shared/lib/utils'
import { formatMoney } from '@/shared/lib/format'
import { useSettingsStore } from '@/shared/store/settings'
import { useVaultStore } from '@/shared/store/vault'
import { useTheme } from '@/shared/lib/theme'
import { sidebarState, useViewport } from '@/shared/lib/viewport'

const { t } = useI18n()
const route = useRoute()
const vault = useVaultStore()
const settingsStore = useSettingsStore()

useTheme()

const width = useViewport()
const sidebar = computed(() => sidebarState(width.value))
const showLabels = computed(() => sidebar.value !== 'rail')
const menuOpen = ref(false)
const settingsModalOpen = ref(false)

// deduped with the identical query inside <SidebarWallets />
const storesQuery = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
  staleTime: 60_000,
})

const favoriteCurrencies = computed(() => settingsStore.favoriteCurrencies)

const isOnline = ref(true)

// D2.4: the background poller never writes transactions, it only stages fills.
// Surface that so the user knows there is something to confirm.
const queryClient = useQueryClient()
const pendingNotice = ref<number | null>(null)
void onEvent<{ staged: number }>('bybit-import-pending', (payload) => {
  queryClient.invalidateQueries({ queryKey: ['pending-imports'] })
  pendingNotice.value = payload.staged
  setTimeout(() => (pendingNotice.value = null), 8000)
})

onMounted(() => {
  isOnline.value = navigator.onLine
  window.addEventListener('online', () => (isOnline.value = true))
  window.addEventListener('offline', () => (isOnline.value = false))
})
onUnmounted(() => {
  window.removeEventListener('online', () => (isOnline.value = true))
  window.removeEventListener('offline', () => (isOnline.value = false))
})

const navItems = [
  { name: 'dashboard', icon: LayoutDashboard, key: 'nav.dashboard' },
  { name: 'transactions', icon: ScrollText, key: 'nav.transactions' },
  { name: 'charts', icon: ChartArea, key: 'nav.charts' },
  { name: 'trading', icon: CandlestickChart, key: 'nav.trading' },
  { name: 'settings', icon: Settings, key: 'nav.settings' },
]

const titleKey = computed(() => (route.meta.i18n as string | undefined) ?? 'app.title')

/** On the wallet page the store name is a more useful title than the nav label. */
const walletTitle = computed(() => {
  if (route.name !== 'wallet') return ''
  const id = String(route.params.id ?? '')
  return storesQuery.data.value?.find((store) => store.id === id)?.name ?? t('nav.wallets')
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

      <nav class="space-y-1 p-2 pb-1">
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

    <button
      v-if="sidebar === 'compact' && menuOpen"
      type="button"
      class="fixed inset-0 z-30 cursor-default bg-black/40"
      :aria-label="t('actions.close')"
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
        <Button
          variant="ghost"
          size="icon"
          class="shrink-0"
          :aria-label="t('nav.settings')"
          :title="t('nav.settings')"
          @click="settingsModalOpen = true"
        >
          <Settings class="size-5" />
        </Button>
        <div v-if="sidebar !== 'compact'" class="flex shrink-0 items-center gap-4 text-sm text-muted-foreground">
          <div class="flex items-center gap-3">
            <Tooltip>
              <TooltipTrigger as-child>
                <div class="flex items-center gap-1.5 px-2 py-1 rounded-md bg-muted hover:bg-accent cursor-pointer transition-colors">
                  <WifiOff v-if="!isOnline" class="size-3.5 text-destructive" />
                  <span class="flex items-center gap-1">
                    <span
                      v-for="curr in favoriteCurrencies"
                      :key="curr"
                      class="px-1.5 py-0.5 text-xs font-medium bg-accent rounded text-accent-foreground"
                    >
                      {{ curr }}
                    </span>
                    <span v-if="!favoriteCurrencies.length" class="px-1.5 py-0.5 text-xs text-muted-foreground">
                      {{ t('settings.favoriteCurrenciesHint') }}
                    </span>
                  </span>
                </div>
              </TooltipTrigger>
              <TooltipContent side="bottom" :side-offset="6">
                <div class="min-w-[160px] space-y-1">
                  <div v-if="!isOnline" class="px-2 py-1 text-center text-xs text-destructive bg-destructive/10 rounded">
                    <WifiOff class="size-3 inline mr-1" /> {{ t('app.offline') }}
                  </div>
                  <div
                    v-if="pendingNotice"
                    class="px-2 py-1 text-center text-xs text-primary bg-primary/10 rounded"
                  >
                    {{ t('import.pendingEvent') }} ({{ pendingNotice }})
                  </div>
                  <div v-if="summary.data.value && favoriteCurrencies.length">
                    <div v-for="curr in favoriteCurrencies" :key="curr" class="px-2 py-1 text-xs flex justify-between">
                      <span class="font-medium">{{ curr }}</span>
                      <span class="tabular-nums text-muted-foreground">
                        {{
                          curr.toLowerCase() === summary.data.value?.base_currency?.toLowerCase()
                            ? formatMoney(summary.data.value.total_base, { currency: curr, decimals: 2 })
                            : curr.toLowerCase() === 'usd'
                              ? formatMoney(summary.data.value.total_usd, { currency: 'usd', decimals: 2 })
                              : `${formatMoney(summary.data.value.total_usd, { currency: curr, decimals: 2 })} (est.)`
                        }}
                      </span>
                    </div>
                  </div>
                  <div v-else-if="!favoriteCurrencies.length" class="px-2 py-1 text-center text-xs text-muted-foreground">
                    {{ t('settings.favoriteCurrencyAdd') }}
                  </div>
                </div>
              </TooltipContent>
            </Tooltip>
            <span class="hidden md:inline-flex items-center gap-1.5 px-2 py-1 text-muted-foreground">
              {{ settingsStore.baseCurrency.toUpperCase() }}
              · {{ t(`settings.theme.${settingsStore.theme}`) }}
            </span>
          </div>
        </div>
      </header>

      <div class="min-h-0 flex-1 overflow-y-auto overflow-x-auto p-4 md:p-6 @container">
        <RouterView />
      </div>
    </main>

    <Dialog
      v-model:open="settingsModalOpen"
      :title="t('settings.appTitle')"
      :description="t('settings.appScopeHint')"
      size="lg"
    >
      <!-- Mounted per open so every visit starts from the file on disk. -->
      <AppSettingsModal v-if="settingsModalOpen" />
      <template #footer>
        <Button @click="settingsModalOpen = false">{{ t('actions.close') }}</Button>
      </template>
    </Dialog>
  </div>
</template>