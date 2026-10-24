<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Settings } from 'lucide-vue-next'

import { AppSettingsDialog } from '@/widgets/app-settings'
import { RecentVaults } from '@/widgets/recent-vaults'
import { SandboxOpenButton } from '@/features/sandbox-open'
import { VaultCreateDialog } from '@/features/vault-create'
import { ReceiveVaultDialog } from '@/features/vault-transfer'
import { VaultUnlockDialog } from '@/features/vault-unlock'
import { Button } from '@/shared/ui/button'
import { useVaultStore } from '@/shared/store/vault'
import { useTour } from '@/shared/lib/tour'

const { t } = useI18n()
const vault = useVaultStore()

const settingsOpen = ref(false)
const createOpen = ref(false)
const receiveOpen = ref(false)
// A vault that just arrived from another device opens right away.
const receivedPath = ref<string | null>(null)
const unlockOpen = ref(false)

// The first visit shows the card, the recent vaults and the sandbox once.
const tour = useTour()
let tourTimer: ReturnType<typeof setTimeout> | undefined
onMounted(() => {
  tourTimer = setTimeout(() => {
    if (!document.querySelector('[role="dialog"]')) tour.autoStart('unlock')
  }, 1200)
})
onBeforeUnmount(() => {
  clearTimeout(tourTimer)
  tour.stop()
})

function onReceived(path: string) {
  void vault.loadRecents().catch(() => {})
  receivedPath.value = path
  unlockOpen.value = true
}
</script>

<template>
  <div class="flex h-full flex-col bg-muted/40 pb-safe">
    <!-- The app shell is not mounted on this public route, so the unlock screen
         carries its own header — otherwise the settings would be unreachable
         before a vault is open. -->
    <header class="box-content flex h-14 shrink-0 items-center gap-2 border-b border-border bg-card px-4 pt-safe md:px-6">
      <div class="flex size-7 shrink-0 items-center justify-center rounded-md bg-primary text-sm font-bold text-primary-foreground">
        M
      </div>
      <span class="truncate text-sm font-semibold">{{ t('app.title') }}</span>
      <Button
        variant="ghost"
        size="icon"
        class="ml-auto shrink-0"
        :aria-label="t('nav.settings')"
        :title="t('nav.settings')"
        @click="settingsOpen = true"
      >
        <Settings class="size-5" />
      </Button>
    </header>

    <div class="flex min-h-0 flex-1 items-start justify-center overflow-y-auto p-4 md:items-center md:p-6">
      <RecentVaults @create="createOpen = true" @receive="receiveOpen = true">
        <template #footer><SandboxOpenButton /></template>
      </RecentVaults>
    </div>

    <VaultCreateDialog v-model:open="createOpen" />
    <ReceiveVaultDialog v-model:open="receiveOpen" @received="onReceived" />
    <VaultUnlockDialog v-if="receivedPath" v-model:open="unlockOpen" :path="receivedPath" />

    <AppSettingsDialog v-model:open="settingsOpen" />
  </div>
</template>
