<script setup lang="ts">
import { computed, ref } from 'vue'
import { useEventListener } from '@vueuse/core'
import { RouterView, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { EnrichmentConsentDialog } from '@/features/enrichment-consent'
import { TransactionDetailsHost } from '@/widgets/transaction-details'
import { ConverterDialog } from '@/features/currency-convert'
import { QuickAddDialog } from '@/features/quick-add'
import { SyncBanner, SyncPairingDialog, SyncPlanDialog, useSyncEvents } from '@/features/vault-sync'
import { SettingsShell } from '@/widgets/settings-shell'
import { WhatsNewDialog } from '@/widgets/whats-new'
import { SidebarInset, SidebarProvider } from '@/shared/ui/sidebar'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { useIsPhone } from '@/shared/lib/phone'
import { useTheme } from '@/shared/lib/theme'
import { useSettingsStore } from '@/shared/store/settings'

import { useBackButton } from './model/use-back-button'
import { useBackendNotices } from './model/use-backend-notices'
import { useCacheEvents } from './model/use-cache-events'
import {
  useConnectivity,
  useNativeMenuBlock,
  useShellActions,
  useTourAutostart,
} from './model/use-shell-state'
import { AppBottomNav } from './ui/app-bottom-nav'
import { AppHeader } from './ui/app-header'
import { AppSidebar } from './ui/app-sidebar'
import { CommandPalette } from './ui/command-palette'

/**
 * The frame around every unlocked screen: sidebar, top bar, the page, and the
 * app-wide dialogs. Background events are handled here once for all screens.
 */
const { t } = useI18n()
const route = useRoute()
const settingsStore = useSettingsStore()

useTheme()
useNativeMenuBlock()
useCacheEvents()
useBackendNotices()
useSyncEvents()
useBackButton()

const { summary, isOnline } = useConnectivity()
const { tour, routeTour } = useTourAutostart()
const { quickAddOpen, openSettings, lock } = useShellActions()

/** On a wallet page the bar names the wallet; elsewhere the screen. */
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const walletName = computed(() => {
  if (route.name !== 'wallet') return null
  return flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === route.params.id)?.store.name ?? null
})
const pageTitle = computed(() => walletName.value ?? t(route.meta.i18n ?? 'app.title'))

const phone = useIsPhone()
/** A full-screen route (the workflow editor) has the phone to itself. */
const fullscreen = computed(() => phone.value && route.meta.fullscreen === true)
/** Typing: the keyboard is up, the bottom bar would only sit on top of it. */
const typing = ref(false)
const TEXT_FIELD = 'input:not([type=checkbox]):not([type=radio]):not([type=range]), textarea, [contenteditable="true"]'
useEventListener(document, 'focusin', (event: FocusEvent) => {
  typing.value = event.target instanceof Element && event.target.matches(TEXT_FIELD)
})
useEventListener(document, 'focusout', () => {
  typing.value = false
})
const bottomNav = computed(() => phone.value && !fullscreen.value && !typing.value)
/** Without the bar below, the page itself keeps clear of the gesture bar. */
const pageClass = computed(() => {
  if (fullscreen.value) return 'overflow-hidden p-0 pt-safe'
  if (bottomNav.value) return 'overflow-y-auto p-3'
  return 'overflow-y-auto p-3 pb-[calc(0.75rem+var(--safe-bottom))] md:p-4 md:pb-[calc(1rem+var(--safe-bottom))]'
})
</script>

<template>
  <SidebarProvider class="h-full min-h-0">
    <AppSidebar @lock="lock" @open-settings="openSettings()" />

    <SidebarInset class="min-h-0 min-w-0 overflow-hidden">
      <AppHeader
        v-if="!fullscreen"
        :title="pageTitle"
        :summary="summary.data.value"
        :online="isOnline"
        :has-tour="tour.hasTour(routeTour)"
        @quick-add="quickAddOpen = true"
        @start-tour="tour.start(routeTour)"
      >
        <template #search>
          <CommandPalette @quick-add="quickAddOpen = true" @settings="openSettings()" @lock="lock" />
        </template>
      </AppHeader>

      <div class="@container min-h-0 flex-1" :class="pageClass">
        <SyncBanner />
        <!-- Keyed by path: /stores/a → /stores/b reuses the page component, and its
             queries and child widgets would otherwise keep showing wallet a. -->
        <RouterView :key="route.path" />
      </div>
      <AppBottomNav v-if="bottomNav" @quick-add="quickAddOpen = true" />
    </SidebarInset>

    <SettingsShell />
    <QuickAddDialog v-model:open="quickAddOpen" />
    <TransactionDetailsHost />
    <ConverterDialog />
    <WhatsNewDialog />
    <SyncPairingDialog />
    <SyncPlanDialog />
    <EnrichmentConsentDialog v-model:open="settingsStore.enrichmentConsentOpen" />
  </SidebarProvider>
</template>
