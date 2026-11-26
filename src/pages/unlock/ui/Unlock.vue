<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { SandboxOpenButton } from '@/features/sandbox-open'
import { VaultCreateDialog } from '@/features/vault-create'
import { Skeleton } from '@/shared/ui/skeleton'
import { BrandMark } from '@/shared/ui/brand-mark'
import { useVaultStore } from '@/shared/store/vault'
import { RecentVaults } from '@/widgets/recent-vaults'

/**
 * The way in: the Display title, the recent vaults as rows, the password (and biometrics) of the chosen one,
 * then `Create new` and the sandbox. The app shell is not mounted on this public route.
 */
const { t } = useI18n()
const vault = useVaultStore()
const createOpen = ref(false)
/** A first run: the list came back empty, i.e. nothing was opened here yet, so the page welcomes instead of asking for a password. */
const firstRun = computed(() => vault.recentsLoaded && vault.recents.length === 0)
</script>

<template>
  <div class="flex h-full flex-col items-center overflow-y-auto bg-background px-4 pt-safe pb-safe md:px-6">
    <main class="grid w-full max-w-sm justify-items-start gap-6 py-8 md:my-auto">
      <header v-if="vault.recentsLoaded" class="grid gap-3">
        <BrandMark class="size-10" />
        <h1 class="text-hero text-balance md:text-display" data-unlock-title>{{ t(firstRun ? 'unlock.welcomeTitle' : 'auth.card.title') }}</h1>
        <p class="text-sm text-muted-foreground">{{ t(firstRun ? 'unlock.welcomeHint' : 'auth.card.hint') }}</p>
      </header>
      <!-- Same height as the header: the title is not known before the list is, and must not jump. -->
      <div v-else class="grid gap-3" data-unlock-skeleton>
        <BrandMark class="size-10" />
        <Skeleton class="h-10 w-[min(16rem,100vw-2rem)]" />
        <Skeleton class="h-5 w-48" />
      </div>
      <RecentVaults @create="createOpen = true">
        <template #footer><SandboxOpenButton /></template>
      </RecentVaults>
    </main>
    <VaultCreateDialog v-model:open="createOpen" />
  </div>
</template>
