<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useIntervalFn, useNow } from '@vueuse/core'
import { RouterLink, RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { BookUser, CircleHelp, CalendarCheck, ChartArea, LayoutDashboard, Lock, Plus, ScrollText, Settings, WifiOff, Clock } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Money } from '@/entities/currency'
import { SettingsShell } from '@/widgets/settings-shell'
import { EnrichmentConsentDialog } from '@/features/enrichment-consent'
import { QuickAddDialog } from '@/features/quick-add'
import { NotificationBell } from '@/widgets/notification-bell'
import { noticeReminderId, noticeRoute } from '@/shared/lib/notice-link'
import { SidebarWallets } from '@/widgets/sidebar-wallets'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
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
import { call, onEvent, parseError } from '@/shared/api/ipc'
import { formatAsOf, formatDate } from '@/shared/lib/format'
import { LEDGER_KEYS, PLANNING_KEYS, rejectText } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { settingsQuery } from '@/shared/lib/settings-link'
import { useTour } from '@/shared/lib/tour'
import { useTheme } from '@/shared/lib/theme'
import { useSettingsStore } from '@/shared/store/settings'
import { useVaultStore } from '@/shared/store/vault'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const vault = useVaultStore()
const settingsStore = useSettingsStore()
const queryClient = useQueryClient()

useTheme()

const quickAddOpen = ref(false)

// The system-wide shortcut (Settings) brings the window up and lands here.
void onEvent('quick-add', () => {
  quickAddOpen.value = true
})

// Deduped with the identical query inside the sidebar widgets.
const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
  staleTime: 60_000,
})

/* --- guided tours: «?» starts the page's tour; a first visit starts it once --- */
const tour = useTour()
const routeTour = computed(() => String(route.name ?? ''))
let tourTimer: ReturnType<typeof setTimeout> | undefined
watch(
  routeTour,
  (id) => {
    // A pending tour belongs to the screen that scheduled it, never to the next one.
    clearTimeout(tourTimer)
    if (!tour.hasTour(id)) return
    // Let the page render its widgets before pointing at them.
    tourTimer = setTimeout(() => {
      if (routeTour.value === id && !document.querySelector('[role="dialog"]')) tour.autoStart(id)
    }, 1200)
  },
  { immediate: true },
)
onUnmounted(() => clearTimeout(tourTimer))

function openSettings(tab = 'general') {
  void router.push({ query: settingsQuery(route.query, tab) })
}

const navItems = [
  { name: 'dashboard', icon: LayoutDashboard, key: 'nav.dashboard' },
  { name: 'transactions', icon: ScrollText, key: 'nav.transactions', prefix: '/transactions' },
  { name: 'planning-automations', icon: CalendarCheck, key: 'nav.planning', prefix: '/planning' },
  { name: 'analytics-cashflow', icon: ChartArea, key: 'nav.analytics', prefix: '/analytics' },
  { name: 'directory-contacts', icon: BookUser, key: 'nav.directory', prefix: '/directory' },
]

/** A wallet page carries its own name and parents; the shell only says where you are. */
const pageTitle = computed(() =>
  route.name === 'wallet' ? t('nav.wallets') : t((route.meta.i18n as string | undefined) ?? 'app.title'),
)

/** File name of the open vault; the full path is in the tooltip. */
const vaultPath = computed(() => vault.status?.path ?? '')
const vaultFile = computed(() => vaultPath.value.split(/[\\/]/).pop() ?? '')


/** Prices older than this are flagged next to the net worth. */
const STALE_PRICES_MS = 60 * 60_000

const isOnline = ref(navigator.onLine)
const now = useNow({ scheduler: (tick) => useIntervalFn(tick, 60_000) })
// Only a real age is reported; "offline" stays a separate, honest signal.
const stalePrices = computed(() => {
  const asOf = summary.data.value?.prices_as_of
  if (!asOf) return null
  const age = now.value.getTime() - new Date(asOf).getTime()
  return age > STALE_PRICES_MS ? formatAsOf(asOf) : null
})
const markOnline = () => (isOnline.value = true)
const markOffline = () => (isOnline.value = false)
/**
 * The WebView's own menu (Reload, Inspect…) is not the app's: every block has
 * its own right-click menu. Text fields keep the native one for copy and paste.
 */
function blockNativeMenu(event: MouseEvent) {
  const target = event.target as HTMLElement | null
  if (target?.closest('input, textarea, [contenteditable="true"], .cm-editor')) return
  event.preventDefault()
}

onMounted(() => {
  window.addEventListener('contextmenu', blockNativeMenu)
  window.addEventListener('online', markOnline)
  window.addEventListener('offline', markOffline)
})
onUnmounted(() => {
  window.removeEventListener('contextmenu', blockNativeMenu)
  window.removeEventListener('online', markOnline)
  window.removeEventListener('offline', markOffline)
})

// The background poller never writes transactions, it only stages fills —
// tell the user there is something to confirm on the exchange wallet.
void onEvent<{ staged: number }>('bybit-import-pending', () => {
  void queryClient.invalidateQueries({ queryKey: ['pending-imports'] })
})

// A refresh that priced operations saved without a price changes their
// badges and every cost basis built on them.
void onEvent<RefreshResult>('prices-updated', (result) => {
  if (!result.restamped) return
  for (const queryKey of [['transactions'], ['store'], ['store-pnl'], ['store-history'], ['history'], ['summary']]) {
    void queryClient.invalidateQueries({ queryKey })
  }
})

/* --- background jobs (plan §7.7): automations, alerts, reminders --- */
function refreshPlanning() {
  for (const queryKey of [...PLANNING_KEYS, ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
}

void onEvent<{ tx_ids: string[] }>('automation-created', () => refreshPlanning())

void onEvent<AutomationRejectedEvent>('automation-rejected', (payload) => {
  refreshPlanning()
  const nodes = flattenStoreTree(queryClient.getQueryData<StoreTreeNode[]>(STORE_TREE_QUERY_KEY) ?? [])
  const storeName = (id: string) => nodes.find((node) => node.store.id === id)?.path.join(' › ') ?? id
  const settle = (cmd: 'retry_automation_run' | 'skip_automation_run') =>
    call(cmd, { runId: payload.run_id })
      .then(() => {
        refreshPlanning()
        toast.success(t(cmd === 'retry_automation_run' ? 'automation.run.retried' : 'automation.run.skipped'))
      })
      .catch((err) =>
        toast.error(t('automation.run.retryFailed'), { description: codeErrorMessage(parseError(err), t) }),
      )
  toast.error(t('automation.event.rejected', { name: payload.name, date: formatDate(payload.planned_at) }), {
    description: rejectText(payload.reason, t, storeName),
    duration: 30_000,
    action: { label: t('automation.run.retry'), onClick: () => void settle('retry_automation_run') },
    cancel: { label: t('automation.run.skip'), onClick: () => void settle('skip_automation_run') },
  })
})

void onEvent<{ count: number }>('automation-skipped', (payload) => {
  refreshPlanning()
  toast.warning(t('automation.event.skipped', { count: payload.count }))
})

void onEvent<{ tx_id: string }>('enrichment-ready', () => {
  void queryClient.invalidateQueries({ queryKey: ['enrichments'] })
  toast.info(t('enrichment.event'))
})

void onEvent<{ alert_id: string }>('price-alert', () => {
  void queryClient.invalidateQueries({ queryKey: ['price-alerts'] })
})

void onEvent('reminder-due', () => void queryClient.invalidateQueries({ queryKey: ['reminders'] }))

/**
 * Every notice the backend shows also lands here: into the bell and as a
 * toast with what can be done right away — open it, mark a reminder done or
 * put it off. A rejected automation has its own toast with «retry / skip».
 */
void onEvent<NoticeRecord>('notification', (record) => {
  void queryClient.invalidateQueries({ queryKey: ['notifications'] })
  if (record.key.startsWith('automation-rejected:')) return
  const target = noticeRoute(record.link)
  const reminderId = noticeReminderId(record.key)
  const settle = (cmd: 'set_reminder_done' | 'snooze_reminder') => {
    const args =
      cmd === 'set_reminder_done'
        ? { id: reminderId, done: true }
        : { input: { id: reminderId, until: new Date(Date.now() + 10 * 60_000).toISOString() } }
    void call(cmd, args)
      .then(() => void queryClient.invalidateQueries({ queryKey: ['reminders'] }))
      .catch((err) => toast.error(t('reminder.saveFailed'), { description: codeErrorMessage(parseError(err), t) }))
  }
  toast.info(record.title, {
    description: record.body,
    duration: reminderId ? 60_000 : 8_000,
    action: reminderId
      ? { label: t('reminder.done'), onClick: () => settle('set_reminder_done') }
      : target
        ? { label: t('actions.open'), onClick: () => void router.push(target) }
        : undefined,
    cancel: reminderId ? { label: t('reminder.snooze.10m'), onClick: () => settle('snooze_reminder') } : undefined,
  })
})

async function lock() {
  await vault.lock()
}

/* --- the tray menu: screens without a page reload, prices, lock --- */
void onEvent<string>('tray-navigate', (target) => {
  if (target === 'settings') openSettings()
  else void router.push({ name: target })
})
void onEvent('refresh-prices', () => {
  void call<RefreshResult>('refresh_all')
    .then((result) => toast.success(t('store.refreshed'), { description: `${result.prices} · ${result.fx}` }))
    .catch((err) => toast.error(t('store.refreshFailed'), { description: parseError(err).message }))
})
void onEvent('toggle-lock', () => void lock())
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
                <SidebarMenuButton as-child :is-active="item.prefix ? route.path.startsWith(item.prefix) : route.name === item.name" :tooltip="t(item.key)">
                  <RouterLink :to="{ name: item.name }">
                    <component :is="item.icon" />
                    <span>{{ t(item.key) }}</span>
                  </RouterLink>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <!-- Settings open over the page, not instead of it. -->
              <SidebarMenuItem>
                <SidebarMenuButton :tooltip="t('nav.settings')" :is-active="typeof route.query.settings === 'string'" @click="openSettings()">
                  <Settings />
                  <span>{{ t('nav.settings') }}</span>
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
        <Badge v-else-if="stalePrices" variant="secondary" class="gap-1" :title="t('app.pricesStaleHint')">
          <Clock class="size-3" />
          {{ t('app.pricesAsOf', { time: stalePrices }) }}
        </Badge>

        <span v-if="summary.data.value" class="hidden text-sm font-medium sm:inline">
          <Money :amount="summary.data.value.total_base" :currency="summary.data.value.base_currency" :decimals="2" />
        </span>

        <Button size="sm" class="shrink-0" :title="t('quickAdd.title')" @click="quickAddOpen = true">
          <Plus class="size-4" />
          <span class="hidden sm:inline">{{ t('quickAdd.button') }}</span>
        </Button>

        <NotificationBell />
        <Button
          v-if="tour.hasTour(routeTour)"
          variant="ghost"
          size="icon"
          class="shrink-0"
          :aria-label="t('tour.start')"
          :title="t('tour.start')"
          data-tour="tour-help"
          @click="tour.start(routeTour)"
        >
          <CircleHelp class="size-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          class="shrink-0"
          :aria-label="t('settings.appTitle')"
          :title="t('settings.appTitle')"
          data-tour="open-settings"
          @click="openSettings()"
        >
          <Settings class="size-4" />
        </Button>
      </header>

      <div class="@container min-h-0 flex-1 overflow-y-auto p-3 md:p-4">
        <!-- Keyed by path: /stores/a → /stores/b reuses the page component, and its
             queries and child widgets would otherwise keep showing wallet a. -->
        <RouterView :key="route.path" />
      </div>
    </SidebarInset>

    <SettingsShell />
    <QuickAddDialog v-model:open="quickAddOpen" />
    <EnrichmentConsentDialog v-model:open="settingsStore.enrichmentConsentOpen" />
  </SidebarProvider>
</template>
