<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronsUpDown, FolderOpen, ShieldOff, Smartphone } from 'lucide-vue-next'

import { VaultDeleteDialog } from '@/features/vault-delete'
import { useVaultForget, useVaultRelocate } from '@/features/vault-forget'
import { VaultUnlockDialog, VaultUnlockForm } from '@/features/vault-unlock'
import { Badge } from '@/shared/ui/badge'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Drawer, DrawerContent, DrawerTitle } from '@/shared/ui/drawer'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { useIsPhone } from '@/shared/lib/phone'
import { useVaultStore } from '@/shared/store/vault'

import RecentVaultList from './RecentVaultList.vue'
import RecentVaultMissing from './RecentVaultMissing.vue'
import { shortDir } from './lib/short-path'

const { t } = useI18n()
const vault = useVaultStore()
const isPhone = useIsPhone()
const { forget, clear } = useVaultForget()
const relocate = useVaultRelocate()

const emit = defineEmits<{ create: []; receive: [] }>()

const unlockOpen = ref(false)
/** The vault the dialog opens; `null` lets the user pick any file. */
const unlockPath = ref<string | null>(null)
/** Open list (popover or sheet). */
const listOpen = ref(false)
const picked = ref<string | null>(null)
/** The in-app vault the question «Удалить хранилище?» is about. */
const deleteTarget = ref<RecentVault | null>(null)
const deleteOpen = ref(false)

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

/** The vault in the field has no file any more. */
const missing = computed(() => current.value !== null && !current.value.exists)

/** The vault in the field has no password: the card shows no field for it. */
const plain = computed(() => current.value !== null && !current.value.encrypted)

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
  remove: (recent: RecentVault) => askDelete(recent),
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

function askDelete(recent: RecentVault) {
  listOpen.value = false
  deleteTarget.value = recent
  deleteOpen.value = true
}

/** «Найти файл…»: the row points at the file found, which the field then shows. */
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
  <!-- One card for the way in (Full-New-Encrypt 3): the vault, its password, «Открыть», then «Создать новое». -->
  <Card class="w-full max-w-80" data-unlock-card data-tour="unlock-card">
    <template #header>
      <h1 class="text-lg font-semibold">{{ t('auth.card.title') }}</h1>
      <p class="text-sm text-muted-foreground">{{ t('auth.card.hint') }}</p>
    </template>

    <div class="grid gap-3">
      <template v-if="vault.recents.length">
        <div class="grid min-w-0 gap-1.5" data-tour="unlock-recents">
          <span class="text-sm font-medium">{{ t('vaultForget.field') }}</span>
          <!-- One list, two holders: a popover under the field, a bottom sheet on a phone. -->
          <Popover :open="!isPhone && listOpen" @update:open="listOpen = $event">
            <PopoverTrigger as-child>
              <Button
                variant="outline"
                :class="cn('h-auto min-h-11 w-full min-w-0 justify-between gap-2 px-3 py-2 text-left', missing && 'border-destructive ring-2 ring-destructive/20')"
                data-recents-field
              >
                <span class="min-w-0 flex-1 truncate text-sm font-medium">
                  {{ current ? nameOf(current) : t('vaultForget.choose') }}
                  <span v-if="current && !current.in_app_folder" class="font-normal text-muted-foreground">
                    · {{ shortDir(current.path) }}
                  </span>
                </span>
                <Badge v-if="current && !current.encrypted" variant="warning" class="shrink-0 gap-1 font-normal" data-recents-plain>
                  <ShieldOff class="size-3" />
                  {{ t('vaultForget.plain') }}
                </Badge>
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
                  {{ t(vault.appStorage ? 'vaultForget.addFromFile' : 'vaultForget.otherFile') }}
                </Button>
              </div>
            </DrawerContent>
          </Drawer>
        </div>
        <RecentVaultMissing
          v-if="missing && current"
          :recent="current"
          :card="isPhone"
          :finding="relocate.isPending.value"
          @find="findFile(current)"
          @forget="forgetRow(current)"
        />
        <VaultUnlockForm v-if="current" :path="current.path" :plain="plain" :disabled="missing" />
      </template>

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
      <Button variant="ghost" class="w-full gap-2 text-muted-foreground" data-unlock-receive @click="emit('receive')">
        <Smartphone class="size-4" />
        {{ t('transfer.receive.cardButton') }}
      </Button>
      <!-- The page puts «Открыть песочницу» (Full-New-Sandbox 1) here. -->
      <slot name="footer" />
    </div>
  </Card>

  <VaultUnlockDialog v-model:open="unlockOpen" :path="unlockPath" />
  <VaultDeleteDialog v-model:open="deleteOpen" :recent="deleteTarget" />
</template>
