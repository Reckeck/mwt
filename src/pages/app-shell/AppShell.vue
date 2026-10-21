<script setup lang="ts">
// check-ui-ignore router-key: keyed by the page record and its params, not by the path
import { computed, ref } from 'vue'
import { useEventListener } from '@vueuse/core'
import { RouterView, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { EnrichmentConsentDialog } from '@/features/enrichment-consent'
import { ConverterDialog } from '@/features/currency-convert'
import { QuickAddDialog } from '@/features/quick-add'
import { SyncBanner, SyncPairingDialog, SyncPlanDialog, useSyncEvents } from '@/features/vault-sync'
import { SettingsShell } from '@/widgets/settings-shell'
import { WhatsNewDialog } from '@/widgets/whats-new'
import { MoneyTooltipHost } from '@/entities/currency'
import { SidebarInset, SidebarProvider } from '@/shared/ui/sidebar'
import { flattenStoreTree } from '@/shared/lib/store-tree'
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
import { EntityHost } from './ui/entity-host'
import { useStoreTree } from '@/shared/api/queries'

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
const tree = useStoreTree()
const walletName = computed(() => {
  if (route.name !== 'wallet') return null
  return flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === route.params.id)?.store.name ?? null
})
const pageTitle = computed(() => walletName.value ?? t(route.meta.i18n ?? 'app.title'))

/** The shell's child page and its params: a section's own sub-routes do not change it. */
const pageKey = computed(() => `${route.matched[1]?.path ?? route.path}?${JSON.stringify(route.params)}`)

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
  if (fullscreen.value) return 'overflow-hidden p-0 pt-safe pb-safe'
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

      <div class="@container min-h-0 flex-1 scrollbar-stable" :class="pageClass">
        <SyncBanner />
        <!-- Keyed by the page and its params, not the path: /stores/a → /stores/b gets a fresh
             page (its widgets keep local state per wallet), while a sub-tab of a section
             (/transactions → /transactions/calendar) keeps the section mounted (rule 17). -->
        <div class="mx-auto w-full" :class="fullscreen ? 'h-full' : 'min-h-full max-w-page'">
          <RouterView :key="pageKey" />
        </div>
      </div>
      <AppBottomNav v-if="bottomNav" @quick-add="quickAddOpen = true" />
    </SidebarInset>

    <SettingsShell />
    <MoneyTooltipHost />
    <QuickAddDialog v-model:open="quickAddOpen" />
    <EntityHost />
    <ConverterDialog />
    <WhatsNewDialog />
    <SyncPairingDialog />
    <SyncPlanDialog />
    <EnrichmentConsentDialog v-model:open="settingsStore.enrichmentConsentOpen" />
  </SidebarProvider>
</template>
