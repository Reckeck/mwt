<script setup lang="ts">
import { watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { Sonner } from '@/shared/ui/sonner'
import { useLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'
import { useSettingsStore, useVaultStore } from '@/shared/store'

const route = useRoute()
const router = useRouter()
const vault = useVaultStore()
const settingsStore = useSettingsStore()

useTheme()
useLocale()

// Loaded before any vault work: the theme and language follow the user, not the
// vault, so they must be readable on the unlock screen — the first thing
// rendered. Vault settings, by contrast, need an unlocked vault and load later.
if (!settingsStore.appLoaded) {
  settingsStore.loadApp().catch(() => {})
}

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

if (settingsStore.vault === null && !vault.locked) {
  settingsStore.loadVault().catch(() => {})
}
</script>

<template>
  <router-view />

  <!-- Mounted here, not in AppShell: `/unlock` is a sibling route, and its
       settings dialog reports saves through toasts. -->
  <Sonner position="bottom-right" rich-colors close-button />
</template>