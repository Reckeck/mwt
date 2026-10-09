<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { FolderPlus, Settings } from 'lucide-vue-next'

import { AppSettingsDialog } from '@/widgets/app-settings'
import { RecentVaults } from '@/widgets/recent-vaults'
import { VaultCreateDialog } from '@/features/vault-create'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'

const { t } = useI18n()

const settingsOpen = ref(false)
const createOpen = ref(false)
</script>

<template>
  <div class="flex h-full flex-col bg-muted/40">
    <!-- The app shell is not mounted on this public route, so the unlock screen
         carries its own header — otherwise the settings would be unreachable
         before a vault is open. -->
    <header class="flex h-14 shrink-0 items-center gap-2 border-b border-border bg-card px-4 md:px-6">
      <div class="flex size-7 shrink-0 items-center justify-center rounded-md bg-primary text-sm font-bold text-primary-foreground">
        T
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
            <p class="text-sm text-muted-foreground">{{ t('auth.createVaultHint') }}</p>
          </template>

          <Button size="lg" class="w-full" @click="createOpen = true">
            <FolderPlus class="size-4" />
            {{ t('auth.createVaultButton') }}
          </Button>
        </Card>
      </div>
    </div>

    <VaultCreateDialog v-model:open="createOpen" />

    <AppSettingsDialog v-model:open="settingsOpen" />
  </div>
</template>
