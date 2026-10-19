<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { FolderPlus, Settings, Smartphone } from 'lucide-vue-next'

import { AppSettingsDialog } from '@/widgets/app-settings'
import { RecentVaults } from '@/widgets/recent-vaults'
import { VaultCreateDialog } from '@/features/vault-create'
import { ReceiveVaultDialog } from '@/features/vault-transfer'
import { VaultUnlockDialog } from '@/features/vault-unlock'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { useVaultStore } from '@/shared/store/vault'

const { t } = useI18n()
const vault = useVaultStore()

const settingsOpen = ref(false)
const createOpen = ref(false)
const receiveOpen = ref(false)
// A vault that just arrived from another device opens right away.
const receivedPath = ref<string | null>(null)
const unlockOpen = ref(false)

function onReceived(path: string) {
  void vault.loadRecents().catch(() => {})
  receivedPath.value = path
  unlockOpen.value = true
}
</script>

<template>
  <div class="flex h-full flex-col bg-muted/40">
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

    <div class="flex min-h-0 flex-1 items-start justify-center overflow-y-auto p-6">
      <div class="grid w-full max-w-md gap-4">
        <RecentVaults />

        <!-- Block 2: create a vault at any path. Always available — it does not
             depend on whether a vault already exists at the default location. -->
        <Card>
          <template #header>
            <div class="flex items-center gap-2">
              <FolderPlus class="size-4 shrink-0 text-muted-foreground" />
              <h2 class="text-base font-semibold">{{ t('auth.createVaultTitle') }}</h2>
            </div>
            <p class="text-sm text-muted-foreground">{{ t(vault.appStorage ? 'auth.createVaultHintApp' : 'auth.createVaultHint') }}</p>
          </template>

          <!-- With a vault to open, creating another is the second choice. -->
          <Button size="lg" class="w-full" :variant="vault.recents.length ? 'outline' : 'default'" @click="createOpen = true">
            <FolderPlus class="size-4" />
            {{ t('auth.createVaultButton') }}
          </Button>
        </Card>

        <Card>
          <template #header>
            <div class="flex items-center gap-2">
              <Smartphone class="size-4 shrink-0 text-muted-foreground" />
              <h2 class="text-base font-semibold">{{ t('transfer.receive.cardTitle') }}</h2>
            </div>
            <p class="text-sm text-muted-foreground">{{ t('transfer.receive.cardHint') }}</p>
          </template>

          <Button size="lg" variant="outline" class="w-full" @click="receiveOpen = true">
            <Smartphone class="size-4" />
            {{ t('transfer.receive.cardButton') }}
          </Button>
        </Card>
      </div>
    </div>

    <VaultCreateDialog v-model:open="createOpen" />
    <ReceiveVaultDialog v-model:open="receiveOpen" @received="onReceived" />
    <VaultUnlockDialog v-if="receivedPath" v-model:open="unlockOpen" :path="receivedPath" />

    <AppSettingsDialog v-model:open="settingsOpen" />
  </div>
</template>
