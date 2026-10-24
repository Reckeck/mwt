<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronsUpDown, FolderOpen, LockKeyhole } from 'lucide-vue-next'

import { VaultUnlockDialog } from '@/features/vault-unlock'
import { useVaultForget } from '@/features/vault-forget'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Drawer, DrawerContent, DrawerTitle } from '@/shared/ui/drawer'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { useIsPhone } from '@/shared/lib/phone'
import { useVaultStore } from '@/shared/store/vault'

import RecentVaultList from './RecentVaultList.vue'
import { shortDir } from './lib/short-path'

const { t } = useI18n()
const vault = useVaultStore()
const isPhone = useIsPhone()
const { forget, clear } = useVaultForget()

const unlockOpen = ref(false)
/** The vault the dialog opens; `null` lets the user pick any file. */
const unlockPath = ref<string | null>(null)
/** Open list (popover or sheet). */
const listOpen = ref(false)
const picked = ref<string | null>(null)

onMounted(async () => {
  // `status` doubles as "is the default location free?" for the create dialog.
  await vault.refreshStatus().catch(() => {})
  await vault.loadRecents()
})

/** The vault in the field: the one picked, else the last opened whose file is still there. */
const current = computed(
  () =>
    vault.recents.find((recent: RecentVault) => recent.path === picked.value) ??
    vault.recents.find((recent: RecentVault) => recent.exists) ??
    null,
)

function nameOf(recent: RecentVault): string {
  return recent.display_name || recent.path
}

function pick(path: string) {
  picked.value = path
  listOpen.value = false
}

const listBindings = computed(() => ({ selected: current.value?.path ?? null }))
const listHandlers = {
  pick: (path: string) => pick(path),
  forget: (recent: RecentVault) => forgetRow(recent),
  clear: () => clearHistory(),
  other: () => openUnlock(null),
}

function openUnlock(path: string | null) {
  listOpen.value = false
  unlockPath.value = path
  unlockOpen.value = true
}

function forgetRow(recent: RecentVault) {
  forget.mutate({ path: recent.path, name: nameOf(recent) })
}

/** Everything but the vault in the field goes; the list stays open. */
function clearHistory() {
  const keep = current.value?.path
  const rest = vault.recents
    .filter((recent: RecentVault) => recent.path !== keep)
    .map((recent: RecentVault) => ({ path: recent.path, name: nameOf(recent) }))
  if (rest.length) clear.mutate(rest)
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
      <template v-if="vault.recents.length">
        <div class="grid gap-1.5">
          <span class="text-sm font-medium">{{ t('vaultForget.field') }}</span>
          <!-- One list, two holders: a popover under the field, a bottom sheet on a phone. -->
          <Popover :open="!isPhone && listOpen" @update:open="listOpen = $event">
            <PopoverTrigger as-child>
              <Button
                variant="outline"
                class="h-auto min-h-11 w-full justify-between gap-2 px-3 py-2 text-left"
                data-recents-field
              >
                <span class="min-w-0 flex-1 truncate text-sm font-medium">
                  {{ current ? nameOf(current) : t('vaultForget.choose') }}
                  <span v-if="current && !vault.appStorage" class="font-normal text-muted-foreground">
                    · {{ shortDir(current.path) }}
                  </span>
                </span>
                <ChevronsUpDown class="size-4 shrink-0 text-muted-foreground" />
              </Button>
            </PopoverTrigger>
            <PopoverContent align="start" class="w-(--reka-popover-trigger-width) p-1">
              <RecentVaultList v-bind="listBindings" v-on="listHandlers" />
            </PopoverContent>
          </Popover>
          <Drawer :open="isPhone && listOpen" @update:open="listOpen = $event">
            <DrawerContent class="gap-2 px-3 pt-2 pb-5">
              <DrawerTitle class="px-2 text-base font-semibold">{{ t('vaultForget.sheetTitle') }}</DrawerTitle>
              <RecentVaultList v-bind="listBindings" sheet v-on="listHandlers" />
              <div class="border-t border-border px-1 pt-3">
                <Button variant="outline" class="h-12 w-full gap-2" data-recents-other @click="openUnlock(null)">
                  <FolderOpen class="size-4" />
                  {{ t(vault.appStorage ? 'auth.importFile' : 'vaultForget.otherFile') }}
                </Button>
              </div>
            </DrawerContent>
          </Drawer>
        </div>
        <Button v-if="current" size="lg" class="w-full" @click="openUnlock(current.path)">
          <LockKeyhole class="size-4" />
          <span class="truncate">{{ t('auth.openLatest', { name: nameOf(current) }) }}</span>
        </Button>
      </template>

      <template v-else>
        <p class="text-sm text-muted-foreground">{{ t('auth.noRecentVaults') }}</p>
        <Button variant="outline" class="w-full justify-start gap-2" @click="openUnlock(null)">
          <FolderOpen class="size-4" />
          {{ t(vault.appStorage ? 'auth.importFile' : 'auth.openOther') }}
        </Button>
      </template>
    </div>
  </Card>

  <VaultUnlockDialog v-model:open="unlockOpen" :path="unlockPath" />
</template>
