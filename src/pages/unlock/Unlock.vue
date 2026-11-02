<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute } from 'vue-router'
import { Settings } from 'lucide-vue-next'

import { AppSettingsDialog } from '@/widgets/app-settings'
import { RecentVaults } from '@/widgets/recent-vaults'
import { SandboxOpenButton } from '@/features/sandbox-open'
import { VaultCreateDialog } from '@/features/vault-create'
import { ReceiveVaultDialog } from '@/features/vault-transfer'
import { useOpenReceivedVault, VaultUnlockDialog } from '@/features/vault-unlock'
import { BrandMark } from '@/shared/ui/brand-mark'
import { Button } from '@/shared/ui/button'
import { useTour } from '@/shared/lib/tour'
import { TourOffer } from '@/widgets/tour-offer'

const { t } = useI18n()
const route = useRoute()
const openReceived = useOpenReceivedVault()

const settingsOpen = ref(false)
const createOpen = ref(false)
const receiveOpen = ref(false)
// A vault that just arrived from another device opens right away.
const receivedPath = ref<string | null>(null)
const unlockOpen = ref(false)

// The first visit offers a tour of the card, the recent vaults and the sandbox (a banner, never by itself).
const tour = useTour()
const offerTour = ref(false)
let tourTimer: ReturnType<typeof setTimeout> | undefined
onMounted(() => {
  tourTimer = setTimeout(() => {
    offerTour.value = !document.querySelector('[role="dialog"]') && tour.shouldOffer('unlock')
  }, 1200)
})
onBeforeUnmount(() => {
  clearTimeout(tourTimer)
  tour.stop()
})

async function onReceived(path: string) {
  if (await openReceived.openIfPlain(path)) return
  receivedPath.value = path
  unlockOpen.value = true
}

// Switched to from the settings: the open vault was locked and the received one waits here.
onMounted(() => {
  const received = route.query.received
  if (typeof received === 'string' && received) void onReceived(received)
})
</script>

<template>
  <div class="flex h-full flex-col bg-muted/40 pb-safe">
    <!-- The app shell is not mounted on this public route, so the unlock screen
         carries its own header — otherwise the settings would be unreachable
         before a vault is open. -->
    <header class="box-content flex h-14 shrink-0 items-center gap-2 border-b border-border bg-card px-4 pt-safe md:px-6">
      <BrandMark class="size-7" />
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

    <TourOffer v-if="offerTour" id="unlock" class="mx-4 mt-3 md:mx-6" @close="offerTour = false" />
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
