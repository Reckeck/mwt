<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { EnrichmentConsentDialog } from '@/features/enrichment-consent'
import { QuickAddDialog } from '@/features/quick-add'
import { SettingsShell } from '@/widgets/settings-shell'
import { WhatsNewDialog } from '@/widgets/whats-new'
import { SidebarInset, SidebarProvider } from '@/shared/ui/sidebar'
import { useTheme } from '@/shared/lib/theme'
import { useSettingsStore } from '@/shared/store/settings'

import { useBackendNotices } from './model/use-backend-notices'
import { useCacheEvents } from './model/use-cache-events'
import {
  useConnectivity,
  useNativeMenuBlock,
  useShellActions,
  useTourAutostart,
} from './model/use-shell-state'
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

const { summary, isOnline } = useConnectivity()
const { tour, routeTour } = useTourAutostart()
const { quickAddOpen, openSettings, lock } = useShellActions()

/** A wallet page carries its own name and parents; the shell only says where you are. */
const pageTitle = computed(() =>
  route.name === 'wallet' ? t('nav.wallets') : t((route.meta.i18n as string | undefined) ?? 'app.title'),
)
</script>

<template>
  <SidebarProvider class="h-full min-h-0">
    <AppSidebar @lock="lock" @open-settings="openSettings()" />

    <SidebarInset class="min-h-0 min-w-0 overflow-hidden">
      <AppHeader
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

      <div class="@container min-h-0 flex-1 overflow-y-auto p-3 md:p-4">
        <!-- Keyed by path: /stores/a → /stores/b reuses the page component, and its
             queries and child widgets would otherwise keep showing wallet a. -->
        <RouterView :key="route.path" />
      </div>
    </SidebarInset>

    <SettingsShell />
    <QuickAddDialog v-model:open="quickAddOpen" />
    <WhatsNewDialog />
    <EnrichmentConsentDialog v-model:open="settingsStore.enrichmentConsentOpen" />
  </SidebarProvider>
</template>
