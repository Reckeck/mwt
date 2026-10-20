<script setup lang="ts">
import { watch } from 'vue'
import { useEventListener } from '@vueuse/core'
import { useRoute, useRouter } from 'vue-router'

import { useIsPhone } from '@/shared/lib/phone'
import { Sonner } from '@/shared/ui/sonner'
import { TooltipProvider } from '@/shared/ui/tooltip'
import { useLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'
import { useSettingsStore, useVaultStore } from '@/shared/store'
import { useVaultSave } from '@/features/vault-save'
import { useBackendEvent } from '@/shared/api/use-backend-event'

const route = useRoute()
const router = useRouter()
const vault = useVaultStore()
const settingsStore = useSettingsStore()

const { resolved: theme } = useTheme()
const phone = useIsPhone()
useLocale()

// Loaded before any vault work: the theme and language follow the user, not the
// vault, so they must be readable on the unlock screen — the first thing
// rendered. Vault settings, by contrast, need an unlocked vault and load later.
if (!settingsStore.appLoaded) {
  settingsStore.loadApp().catch(() => {})
  // Pickers open instantly when the catalog is already here; the backend serves a fresh cache without the network.
  settingsStore.loadCurrencies().catch(() => {})
}

// The backend may lock the vault by itself (Android: after a while in the
// background, or from the sync notification): the window follows, and
// checks again when it comes back in case the event came while it slept.
useBackendEvent<VaultStatus>('vault-changed', (status) => vault.applyStatus(status))
// Batched saves: written when the window hides; a failed write offers to retry.
useVaultSave()
useEventListener(document, 'visibilitychange', () => {
  if (document.visibilityState === 'visible') vault.refreshStatus().catch(() => {})
})

router.beforeEach(async (to) => {
  const wantsPublic = to.meta.public === true
  if (vault.status === null) {
    try {
      await vault.refreshStatus()
    } catch {
      /* offline dev/mock — keep unlocked flag fallback */
    }
  }
  if (!wantsPublic && vault.locked) return { name: 'unlock' }
  if (wantsPublic && !vault.locked) return { name: 'dashboard' }
  return true
})

watch(
  () => vault.locked,
  (locked) => {
    if (locked && route.name !== 'unlock') {
      void router.push({ name: 'unlock' })
    }
  },
)

// Vault settings exist only while a vault is open: load them on every unlock
// (not just when the app starts already unlocked, which it never does after a
// cold start) and forget them on lock so the next vault does not inherit them.
watch(
  () => vault.locked,
  (locked) => {
    if (locked) settingsStore.vault = null
    else settingsStore.loadVault().catch(() => {})
  },
  { immediate: true },
)
</script>

<template>
  <!-- One provider for every tooltip, including the unlock screen outside the shell. -->
  <TooltipProvider :delay-duration="300">
    <router-view />
  </TooltipProvider>

  <!-- Mounted here, not in AppShell: `/unlock` is a sibling route, and its
       settings dialog reports saves through toasts. -->
  <!-- On a phone the toasts drop from the top, below the status bar, clear of the bottom navigation. -->
  <Sonner
    :position="phone ? 'top-center' : 'bottom-right'"
    :offset="phone ? 'calc(var(--safe-top) + 12px)' : undefined"
    :mobile-offset="{ top: 'calc(var(--safe-top) + 12px)' }"
    :theme="theme"
    rich-colors
    close-button
  />
</template>