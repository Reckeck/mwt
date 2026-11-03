<script setup lang="ts">
// check-ui-ignore router-key: keyed by the page record and its params, not by the path
import { computed, onScopeDispose, ref } from 'vue'
import { useEventListener } from '@vueuse/core'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { ContactEditDialog, useContactCreation } from '@/features/contact-edit'
import { EnrichmentConsentDialog } from '@/features/enrichment-consent'
import { ConverterDialog } from '@/features/currency-convert'
import { QuickAddDialog } from '@/widgets/quick-add'
import { AutomationGlance, WalletGlance } from '@/widgets/quick-glance'
import { UpdateNotice, useUpdateEvents } from '@/features/app-update'
import { SyncBanner, SyncPairingDialog, SyncPlanDialog, useSyncEvents } from '@/features/vault-sync'
import { SettingsShell } from '@/widgets/settings-shell'
import { TourOffer } from '@/widgets/tour-offer'
import { WhatsNewDialog } from '@/widgets/whats-new'
import { MoneyCardHost, MoneyTooltipHost } from '@/entities/currency'
import { SidebarInset, SidebarProvider } from '@/shared/ui/sidebar'
import { registerDefaultBack } from '@/shared/lib/drill'
import { withOpenStack } from '@/shared/lib/entity-link'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { SECTION_HOTKEYS, useHotkey } from '@/shared/lib/hotkeys'
import { useIsPhone } from '@/shared/lib/phone'
import { useTheme } from '@/shared/lib/theme'
import { provideContactCreator } from '@/shared/lib/contact-create'
import { provideTxGlances } from '@/shared/lib/tx-glances'
import { useSettingsStore } from '@/shared/store/settings'
import { useVaultStore } from '@/shared/store/vault'

import { NAV_ITEMS } from './app-shell.constants'
import { useBackButton } from './model/use-back-button'
import { useBackendNotices } from './model/use-backend-notices'
import { useCacheEvents } from './model/use-cache-events'
import { useRecordPlaces } from './model/use-recent-places'
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
import { SandboxBar } from './ui/sandbox-bar'
import { useStoreTree } from '@/shared/api/queries'

/**
 * The frame around every unlocked screen: sidebar, top bar, the page, and the
 * app-wide dialogs. Background events are handled here once for all screens.
 */
// Every list of operations below makes a wallet's name and `Auto` a glance.
provideTxGlances({ wallet: WalletGlance, automation: AutomationGlance })
// A contact typed into any picker is made in one form hosted here, over whatever is open.
const contactCreation = useContactCreation()
provideContactCreator(contactCreation.create)
const { t } = useI18n()
const route = useRoute()
const settingsStore = useSettingsStore()
const vault = useVaultStore()

useTheme()
useNativeMenuBlock()
useCacheEvents()
useBackendNotices()
useSyncEvents()
useUpdateEvents()
useBackButton()

const { summary, isOnline } = useConnectivity()
const { tour, routeTour, offer } = useTourAutostart()
const { quickAddOpen, openSettings, lock } = useShellActions()

/** On a wallet page the bar names the wallet; elsewhere the screen. */
const tree = useStoreTree()
const walletName = computed(() => {
  if (route.name !== 'wallet') return null
  return flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === route.params.id)?.store.name ?? null
})
// Home says `No connection` in its own bar, with the rates' age (Currency-2 · 8); the red badge is for the other screens.
const headerOnline = computed(() => isOnline.value || route.name === 'dashboard')
const pageTitle = computed(() => walletName.value ?? t(route.meta.i18n ?? 'app.title'))
useRecordPlaces(pageTitle)
useHotkey('quickAdd', () => (quickAddOpen.value = true))
const router = useRouter()
/** A jump to a list of operations from here offers «← <this screen>» (docs/ui-links.md, 22–23); an open card is not part of the way back. */
onScopeDispose(
  registerDefaultBack(() => ({
    to: router.resolve({ path: route.path, query: withOpenStack(route.query, []) }).fullPath,
    label: pageTitle.value,
  })),
)
/** Alt 1…5: a section without the search (Paths 1 · 7). */
for (const [index, id] of SECTION_HOTKEYS.entries()) useHotkey(id, () => void router.push({ name: NAV_ITEMS[index]?.name ?? 'dashboard' }))

/** The shell's child page and its params: a section's own sub-routes do not change it. */
const pageKey = computed(() => `${route.matched[1]?.path ?? route.path}?${JSON.stringify(route.params)}`)

const phone = useIsPhone()
/** A full-screen route (the workflow editor) has the phone to itself; on a desktop it fills the window without scrolling. */
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
  // The sandbox bar above takes the status bar's inset itself.
  if (fullscreen.value) return ['overflow-hidden p-0 pb-safe', !vault.sandbox && 'pt-safe']
  if (bottomNav.value) return 'overflow-y-auto p-3'
  return 'overflow-y-auto p-3 pb-[calc(0.75rem+var(--safe-bottom))] md:p-4 md:pb-[calc(1rem+var(--safe-bottom))]'
})
</script>

<template>
  <SidebarProvider class="h-full min-h-0">
    <AppSidebar @lock="lock" @open-settings="openSettings()" />

    <SidebarInset class="min-h-0 min-w-0 overflow-hidden">
      <SandboxBar v-if="vault.sandbox" />
      <AppHeader
        v-if="!fullscreen"
        :title="pageTitle"
        :summary="summary.data.value"
        :online="headerOnline"
        :has-tour="tour.hasTour(routeTour)"
        :tour-steps="tour.stepCount(routeTour)"
        :covered="vault.sandbox"
        @quick-add="quickAddOpen = true"
        @start-tour="tour.start(routeTour)"
        @all-tours="openSettings('help')"
      >
        <template #search>
          <CommandPalette @quick-add="quickAddOpen = true" @settings="openSettings()" @lock="lock" />
        </template>
      </AppHeader>

      <div class="@container min-h-0 flex-1 scrollbar-stable" :class="pageClass">
        <!-- Floats under the header: a banner arriving must not push the page down (rule 17). -->
        <div class="banner-stack pointer-events-none fixed inset-x-0 z-banner flex flex-col items-center gap-2 px-4">
          <div class="grid w-full max-w-md gap-2">
            <SyncBanner />
            <UpdateNotice />
          </div>
        </div>
        <TourOffer v-if="offer" :id="offer" @close="offer = null" />
        <!-- Keyed by the page and its params, not the path: /stores/a → /stores/b gets a fresh
             page (its widgets keep local state per wallet), while a sub-tab of a section
             (/transactions → /transactions/calendar) keeps the section mounted (rule 17). -->
        <div class="mx-auto w-full" :class="route.meta.fullscreen ? 'h-full' : 'min-h-full max-w-page'">
          <RouterView :key="pageKey" />
        </div>
      </div>
      <AppBottomNav v-if="bottomNav" @quick-add="quickAddOpen = true" @lock="lock" @open-settings="openSettings()" />
    </SidebarInset>

    <SettingsShell />
    <MoneyTooltipHost />
    <MoneyCardHost />
    <QuickAddDialog v-model:open="quickAddOpen" />
    <EntityHost />
    <ConverterDialog />
    <ContactEditDialog
      v-model:open="contactCreation.open.value"
      :initial-name="contactCreation.name.value"
      @saved="contactCreation.saved"
    />
    <WhatsNewDialog />
    <SyncPairingDialog />
    <SyncPlanDialog />
    <EnrichmentConsentDialog v-model:open="settingsStore.enrichmentConsentOpen" />
  </SidebarProvider>
</template>

<style scoped>
/* Under the header (3.5rem + the status bar); the header's height lives in AppHeader. */
.banner-stack {
  top: calc(var(--safe-top) + 4rem);
}
</style>
