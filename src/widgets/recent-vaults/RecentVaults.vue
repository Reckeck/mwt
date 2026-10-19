<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { FolderOpen, Link2, LockKeyhole, X } from 'lucide-vue-next'

import { VaultUnlockDialog } from '@/features/vault-unlock'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { notify } from '@/shared/lib/notify'
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

/** Only the entry in the list goes; the file itself stays where it is. */
async function forget(path: string) {
  const name = displayName(path)
  try {
    await vault.forgetRecent(path)
    notify.success('auth.forgotten', { description: name })
  } catch (err) {
    notify.error(err, 'auth.forgetFailed')
  }
}

function displayName(path: string): string {
  return vault.recents.find((recent: RecentVault) => recent.path === path)?.display_name || path
}

/** The vault opened last, when its file is still there: the one big button. */
const latest = computed(() => vault.recents.find((recent: RecentVault) => recent.exists) ?? null)

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
      <Button v-if="latest" size="lg" class="w-full" @click="openUnlock(latest.path)">
        <LockKeyhole class="size-4" />
        <span class="truncate">{{ t('auth.openLatest', { name: latest.display_name || latest.path }) }}</span>
      </Button>
      <div v-if="vault.recents.length" class="grid max-h-64 gap-1.5 overflow-y-auto">
        <div v-for="recent in vault.recents" :key="recent.path" class="flex items-center gap-1 rounded-md border border-border">
          <Button
            variant="ghost"
            size="block"
            class="rounded-r-none flex min-w-0 flex-1 items-center gap-2 rounded-l-md px-3 py-2 text-left hover:bg-accent hover:text-accent-foreground"
            :class="{ 'text-destructive': !recent.exists }"
            @click="openUnlock(recent.path)"
          >
            <div class="min-w-0 flex-1">
              <p class="flex items-center gap-1.5 truncate text-sm font-medium">
                <span class="truncate">{{ recent.display_name || recent.path }}</span>
                <Link2 v-if="recent.linked" class="size-3.5 shrink-0 text-muted-foreground" :aria-label="t('vaultLink.linkedBadge')" />
              </p>
              <!-- In app storage the path tells the user nothing. -->
              <p v-if="!vault.appStorage" class="truncate text-xs text-muted-foreground">{{ recent.path }}</p>
            </div>
            <span v-if="!recent.exists" class="shrink-0 text-xs text-destructive">{{ t('auth.missing') }}</span>
          </Button>
          <!-- App storage lists every vault on the device: there is no list entry to forget. -->
          <ConfirmButton
            v-if="!vault.appStorage"
            :label="t('auth.forget')"
            :confirm-label="t('auth.forgetConfirm')"
            @confirm="forget(recent.path)"
          >
            <X class="size-3.5" />
          </ConfirmButton>
        </div>
      </div>

      <p v-else class="text-sm text-muted-foreground">{{ t('auth.noRecentVaults') }}</p>

      <Button variant="outline" class="w-full justify-start gap-2" @click="openUnlock(null)">
        <FolderOpen class="size-4" />
        {{ t(vault.appStorage ? 'auth.importFile' : 'auth.openOther') }}
      </Button>
    </div>
  </Card>

  <VaultUnlockDialog v-model:open="unlockOpen" :path="unlockPath" />
</template>
