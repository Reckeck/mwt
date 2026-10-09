<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { FolderOpen, LockKeyhole, X } from 'lucide-vue-next'

import { VaultUnlockDialog } from '@/features/vault-unlock'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { useVaultStore } from '@/shared/store/vault'

const { t } = useI18n()
const vault = useVaultStore()

const unlockOpen = ref(false)
/** The vault the dialog opens; `null` lets the user pick any file. */
const unlockPath = ref<string | null>(null)

onMounted(async () => {
  // `status` doubles as "is the default location free?" for the create dialog.
  await vault.refreshStatus().catch(() => {})
  await vault.loadRecents()
})

function openUnlock(path: string | null) {
  unlockPath.value = path
  unlockOpen.value = true
}
</script>

<template>
  <!-- Open one of the vaults seen before, or any other file. -->
  <Card>
    <template #header>
      <div class="flex items-center gap-2">
        <LockKeyhole class="size-4 shrink-0 text-muted-foreground" />
        <h2 class="text-base font-semibold">{{ t('auth.recentVaults') }}</h2>
      </div>
    </template>

    <div class="grid gap-3">
      <div v-if="vault.recents.length" class="grid max-h-64 gap-1.5 overflow-y-auto">
        <div v-for="recent in vault.recents" :key="recent.path" class="flex items-center gap-1 rounded-md border border-border">
          <button
            type="button"
            class="flex min-w-0 flex-1 items-center gap-2 rounded-l-md px-3 py-2 text-left hover:bg-accent hover:text-accent-foreground"
            :class="{ 'text-destructive': !recent.exists }"
            @click="openUnlock(recent.path)"
          >
            <div class="min-w-0 flex-1">
              <p class="truncate text-sm font-medium">{{ recent.path.split('/').pop() || recent.path }}</p>
              <p class="truncate text-xs text-muted-foreground">{{ recent.path }}</p>
            </div>
            <span v-if="!recent.exists" class="shrink-0 text-xs text-destructive">{{ t('auth.missing') }}</span>
          </button>
          <Button
            variant="ghost"
            size="icon"
            class="size-8 shrink-0 text-muted-foreground"
            :aria-label="t('auth.forget')"
            :title="t('auth.forget')"
            @click="vault.forgetRecent(recent.path)"
          >
            <X class="size-3.5" />
          </Button>
        </div>
      </div>

      <p v-else class="text-sm text-muted-foreground">{{ t('auth.noRecentVaults') }}</p>

      <Button variant="outline" class="w-full justify-start gap-2" @click="openUnlock(null)">
        <FolderOpen class="size-4" />
        {{ t('auth.openOther') }}
      </Button>
    </div>
  </Card>

  <VaultUnlockDialog v-model:open="unlockOpen" :path="unlockPath" />
</template>
