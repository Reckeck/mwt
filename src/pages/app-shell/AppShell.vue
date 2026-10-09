<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { RouterLink, RouterView, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { ChartArea, LayoutDashboard, Lock, ScrollText, Settings, WifiOff } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { AppSettingsModal } from '@/widgets/app-settings'
import { SidebarWallets } from '@/widgets/sidebar-wallets'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Separator } from '@/shared/ui/separator'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarRail,
  SidebarSeparator,
  SidebarTrigger,
} from '@/shared/ui/sidebar'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'
import { call, onEvent } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { useTheme } from '@/shared/lib/theme'
import { useVaultStore } from '@/shared/store/vault'

const { t } = useI18n()
const route = useRoute()
const vault = useVaultStore()
const queryClient = useQueryClient()

useTheme()

const settingsOpen = ref(false)

// Deduped with the identical query inside the sidebar widgets.
const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
  staleTime: 60_000,
})

const navItems = [
  { name: 'dashboard', icon: LayoutDashboard, key: 'nav.dashboard' },
  { name: 'transactions', icon: ScrollText, key: 'nav.transactions' },
  { name: 'charts', icon: ChartArea, key: 'nav.charts' },
  { name: 'settings', icon: Settings, key: 'nav.settings' },
]

/** A wallet page carries its own name and parents; the shell only says where you are. */
const pageTitle = computed(() =>
  route.name === 'wallet' ? t('nav.wallets') : t((route.meta.i18n as string | undefined) ?? 'app.title'),
)

/** File name of the open vault; the full path is in the tooltip. */
const vaultPath = computed(() => vault.status?.path ?? '')
const vaultFile = computed(() => vaultPath.value.split(/[\\/]/).pop() ?? '')

const netWorth = computed(() => {
  const data = summary.data.value
  if (!data) return null
  return {
    base: formatMoney(data.total_base, { currency: data.base_currency, decimals: 2 }),
    usd: formatMoney(data.total_usd, { currency: 'usd', decimals: 2 }),
    sameCurrency: data.base_currency.toLowerCase() === 'usd',
  }
})

const isOnline = ref(navigator.onLine)
const markOnline = () => (isOnline.value = true)
const markOffline = () => (isOnline.value = false)
onMounted(() => {
  window.addEventListener('online', markOnline)
  window.addEventListener('offline', markOffline)
})
onUnmounted(() => {
  window.removeEventListener('online', markOnline)
  window.removeEventListener('offline', markOffline)
})

// The background poller never writes transactions, it only stages fills —
// tell the user there is something to confirm on the exchange wallet.
void onEvent<{ staged: number }>('bybit-import-pending', (payload) => {
  void queryClient.invalidateQueries({ queryKey: ['pending-imports'] })
  toast.info(t('import.pendingEvent'), { description: String(payload.staged) })
})

async function lock() {
  await vault.lock()
}
</script>

<template>
  <SidebarProvider class="h-full min-h-0">
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton as-child size="lg" :tooltip="vaultPath || t('app.title')">
              <RouterLink :to="{ name: 'dashboard' }">
                <div
                  class="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sm font-bold text-sidebar-primary-foreground"
                >
                  T
                </div>
                <div class="grid flex-1 text-left leading-tight">
                  <span class="truncate font-semibold">{{ t('app.title') }}</span>
                  <span class="truncate text-xs text-muted-foreground" :title="vaultPath">{{ vaultFile }}</span>
                </div>
              </RouterLink>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem v-for="item in navItems" :key="item.name">
                <SidebarMenuButton as-child :is-active="route.name === item.name" :tooltip="t(item.key)">
                  <RouterLink :to="{ name: item.name }">
                    <component :is="item.icon" />
                    <span>{{ t(item.key) }}</span>
                  </RouterLink>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <SidebarSeparator />

        <SidebarWallets />
      </SidebarContent>

      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton :tooltip="t('auth.lock')" @click="lock">
              <Lock />
              <span>{{ t('auth.lock') }}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>

      <SidebarRail />
    </Sidebar>

    <SidebarInset class="min-h-0 min-w-0 overflow-hidden">
      <header class="flex h-14 shrink-0 items-center gap-2 border-b border-border px-4 md:px-6">
        <SidebarTrigger class="-ml-1" />
        <Separator orientation="vertical" class="mr-2 data-[orientation=vertical]:h-4" />
        <h1 class="min-w-0 flex-1 truncate text-base font-semibold">{{ pageTitle }}</h1>

        <Badge v-if="!isOnline" variant="destructive" class="gap-1">
          <WifiOff class="size-3" />
          {{ t('app.offline') }}
        </Badge>

        <Tooltip v-if="netWorth">
          <TooltipTrigger as-child>
            <span class="hidden text-sm font-medium tabular-nums sm:inline">{{ netWorth.base }}</span>
          </TooltipTrigger>
          <TooltipContent v-if="!netWorth.sameCurrency" side="bottom">≈ {{ netWorth.usd }}</TooltipContent>
        </Tooltip>

        <Button
          variant="ghost"
          size="icon"
          class="shrink-0"
          :aria-label="t('settings.appTitle')"
          :title="t('settings.appTitle')"
          @click="settingsOpen = true"
        >
          <Settings class="size-4" />
        </Button>
      </header>

      <div class="@container min-h-0 flex-1 overflow-y-auto p-4 md:p-6">
        <!-- Keyed by path: /stores/a → /stores/b reuses the page component, and its
             queries and child widgets would otherwise keep showing wallet a. -->
        <RouterView :key="route.path" />
      </div>
    </SidebarInset>

    <Dialog
      v-model:open="settingsOpen"
      :title="t('settings.appTitle')"
      :description="t('settings.appScopeHint')"
      size="lg"
    >
      <!-- Mounted per open so every visit starts from the file on disk. -->
      <AppSettingsModal v-if="settingsOpen" />
      <template #footer>
        <Button @click="settingsOpen = false">{{ t('actions.close') }}</Button>
      </template>
    </Dialog>
  </SidebarProvider>
</template>
