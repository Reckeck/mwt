<script setup lang="ts">
import { watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { useSettingsStore } from '@/stores/settings'
import { useVaultStore } from '@/stores/vault'
import { useLocale } from '@/locale'
import { useTheme } from '@/theme'

const route = useRoute()
const router = useRouter()
const vault = useVaultStore()
const settingsStore = useSettingsStore()

useTheme()
useLocale()

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

if (settingsStore.settings === null) {
  settingsStore.load().catch(() => {})
}
</script>

<template>
  <router-view />
</template>