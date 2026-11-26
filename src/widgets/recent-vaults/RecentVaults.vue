<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { FolderOpen } from 'lucide-vue-next'

import { VaultDeleteDialog } from '@/features/vault-delete'
import { useVaultForget, useVaultRelocate } from '@/features/vault-forget'
import { VaultUnlockDialog, VaultUnlockForm } from '@/features/vault-unlock'
import { Button } from '@/shared/ui/button'
import { useVaultStore } from '@/shared/store/vault'

import RecentVaultList from './RecentVaultList.vue'
import RecentVaultMissing from './RecentVaultMissing.vue'

const { t } = useI18n()
const vault = useVaultStore()
const { forget, clear } = useVaultForget()
const relocate = useVaultRelocate()

const emit = defineEmits<{ create: [] }>()

const unlockOpen = ref(false)
/** The vault the dialog opens; `null` lets the user pick any file. */
const unlockPath = ref<string | null>(null)
const picked = ref<string | null>(null)
/** The in-app vault the question `Delete vault` is about. */
const deleteTarget = ref<RecentVault | null>(null)
const deleteOpen = ref(false)

onMounted(async () => {
  // `status` doubles as "is the default location free?" for the create dialog.
  await vault.refreshStatus().catch(() => {})
  await load()
})

/** A failed read shows the error with a retry, never the first-run welcome. */
async function load() {
  await vault.loadRecents().catch(() => {})
}

/** The vault in the field: the one picked, else the last opened whose file is still there. */
const current = computed(
  () =>
    vault.recents.find((recent: RecentVault) => recent.path === picked.value) ??
    vault.recents.find((recent: RecentVault) => recent.exists) ??
    null,
)

/** The vault in the field has no file any more. */
const missing = computed(() => current.value !== null && !current.value.exists)

/** The vault in the field has no password: the card shows no field for it. */
const plain = computed(() => current.value !== null && !current.value.encrypted)

function nameOf(recent: RecentVault): string {
  return recent.display_name || recent.path
}

function pick(path: string) {
  picked.value = path
}


function openUnlock(path: string | null) {
  unlockPath.value = path
  unlockOpen.value = true
}

function forgetRow(recent: RecentVault) {
  forget.mutate({ path: recent.path, name: nameOf(recent) })
}

function askDelete(recent: RecentVault) {
  deleteTarget.value = recent
  deleteOpen.value = true
}

/** `Find file…`: the row points at the file found, which the field then shows. */
function findFile(recent: RecentVault) {
  relocate.mutate(recent.path, {
    onSuccess: (found) => {
      if (found) picked.value = found
    },
  })
}

/** Everything but the vault in the field goes; the list stays open. */
function clearHistory() {
  const keep = current.value?.path
  const rest = vault.recents
    .filter((recent: RecentVault) => recent.path !== keep && !recent.in_app_folder)
    .map((recent: RecentVault) => ({ path: recent.path, name: nameOf(recent) }))
  if (rest.length) clear.mutate(rest)
}
</script>

<template>
  <section class="grid w-full max-w-sm gap-4" data-unlock-card data-tour="unlock-card">
    <template v-if="vault.recents.length">
      <div class="grid min-w-0 gap-1" data-tour="unlock-recents">
        <h2 class="heading-card">{{ t('vaultForget.field') }}</h2>
        <RecentVaultList
          :selected="current?.path ?? null"
          @pick="pick"
          @forget="forgetRow"
          @remove="askDelete"
          @clear="clearHistory"
          @other="openUnlock(null)"
        />
      </div>
      <RecentVaultMissing
        v-if="missing && current"
        :recent="current"
        card
        :finding="relocate.isPending.value"
        @find="findFile(current)"
        @forget="forgetRow(current)"
      />
      <VaultUnlockForm v-if="current" :path="current.path" :plain="plain" :disabled="missing" />
    </template>

    <div v-else-if="vault.recentsError" class="grid justify-items-start gap-2" data-recents-error>
      <p class="text-sm text-destructive">{{ t('auth.recentsFailed') }}</p>
      <Button variant="outline" data-recents-retry @click="load">{{ t('actions.retry') }}</Button>
    </div>

    <template v-else>
      <p class="text-sm text-muted-foreground">{{ t('auth.noRecentVaults') }}</p>
      <Button variant="outline" class="w-full justify-start gap-2" @click="openUnlock(null)">
        <FolderOpen class="size-4" />
        {{ t(vault.appStorage ? 'auth.importFile' : 'auth.openOther') }}
      </Button>
    </template>

    <div class="flex flex-wrap items-center justify-between gap-x-3 pt-1 text-sm">
      <span class="text-muted-foreground">{{ t('auth.noVaultYet') }}</span>
      <Button variant="link" class="h-auto min-h-10 px-0" data-unlock-create @click="emit('create')">
        {{ t('auth.createNew') }}
      </Button>
    </div>
    <!-- The page puts `Open the sandbox` (Full-New-Sandbox 1) here. -->
    <slot name="footer" />
  </section>

  <VaultUnlockDialog v-model:open="unlockOpen" :path="unlockPath" />
  <VaultDeleteDialog v-model:open="deleteOpen" :recent="deleteTarget" />
</template>
