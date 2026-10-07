<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { FolderOpen, FolderPlus, LockKeyhole, Settings, X } from 'lucide-vue-next'

import AppSettingsModal from '@/components/settings/AppSettingsModal.vue'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Dialog from '@/components/ui/dialog.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { parseError } from '@/lib/ipc'
import { useVaultStore } from '@/stores/vault'

const { t } = useI18n()
const router = useRouter()
const vault = useVaultStore()

const settingsOpen = ref(false)

// -- unlock dialog ------------------------------------------------------------
const unlockOpen = ref(false)
const unlockPassword = ref('')
const unlockError = ref<string | null>(null)
const unlockBusy = ref(false)
/** `true` when the vault came from the recents list and the path is fixed. */
const unlockPathFixed = ref(false)
const unlockTargetPath = ref('')

const unlockPath = computed(() => unlockTargetPath.value.trim())

// -- create dialog ------------------------------------------------------------
const createOpen = ref(false)
const createPath = ref('')
const createPassword = ref('')
const createPasswordRepeat = ref('')
const createError = ref<string | null>(null)
const createBusy = ref(false)

const passwordMatch = computed(() => createPasswordRepeat.value.length > 0 && createPasswordRepeat.value === createPassword.value)

onMounted(async () => {
  // `status` doubles as "is the default location free?" for the create dialog.
  await vault.refreshStatus().catch(() => {})
  await vault.loadRecents()
})

// -- dialog openers -----------------------------------------------------------

function openUnlock(path: string) {
  unlockPathFixed.value = true
  unlockTargetPath.value = path
  unlockPassword.value = ''
  unlockError.value = null
  unlockOpen.value = true
}

/** Opens the dialog in "pick any file" mode: the path becomes editable. */
function openUnlockOther() {
  unlockPathFixed.value = false
  unlockTargetPath.value = ''
  unlockPassword.value = ''
  unlockError.value = null
  unlockOpen.value = true
}

async function openCreate() {
  createPassword.value = ''
  createPasswordRepeat.value = ''
  createError.value = null
  createOpen.value = true
  // Pre-fill the default location only while it is still free — suggesting a
  // path that already holds a vault would just fail with ALREADY_EXISTS.
  try {
    const status = vault.status ?? (await vault.refreshStatus())
    createPath.value = status?.exists ? '' : (status?.path ?? '')
  } catch {
    createPath.value = ''
  }}

/**
 * Reads an `AppError` into a translated message. `SECRET` and `ALREADY_EXISTS`
 * are the two codes the vault commands produce that deserve their own wording.
 */
function vaultErrorMessage(err: unknown): string {
  const { code, message } = parseError(err)
  if (code === 'SECRET') return t('auth.wrongPassword')
  if (code === 'ALREADY_EXISTS') return t('auth.pathTaken')
  if (code === 'UNSUPPORTED') return t('auth.unsupported')
  if (code === 'NOT_FOUND') return t('auth.missing')
  return message
}

// -- unlock -------------------------------------------------------------------

async function browseUnlockPath() {
  try {
    const picked = await vault.pickVaultFile(false)
    if (picked) unlockTargetPath.value = picked
  } catch (err) {
    unlockError.value = vaultErrorMessage(err)
  }
}

async function submitUnlock() {
  if (!unlockPath.value) {
    unlockError.value = t('auth.pathRequired')
    return
  }
  unlockBusy.value = true
  unlockError.value = null
  try {
    await vault.unlock(unlockPath.value, unlockPassword.value)
    await router.replace({ name: 'dashboard' })
  } catch (err) {
    unlockError.value = vaultErrorMessage(err)
  } finally {
    unlockBusy.value = false
  }
}

// -- create -------------------------------------------------------------------

async function browseCreatePath() {
  try {
    const picked = await vault.pickVaultFile(true)
    if (picked) createPath.value = picked
  } catch (err) {
    createError.value = vaultErrorMessage(err)
  }
}

async function submitCreate() {
  const path = createPath.value.trim()
  if (!path) {
    createError.value = t('auth.pathRequired')
    return
  }
  if (createPassword.value.length < 4) {
    createError.value = t('auth.passwordTooShort')
    return
  }
  if (createPassword.value !== createPasswordRepeat.value) {
    createError.value = t('auth.passwordMismatch')
    return
  }
  createBusy.value = true
  createError.value = null
  try {
    await vault.createVault(path, createPassword.value)
    await router.replace({ name: 'dashboard' })
  } catch (err) {
    createError.value = vaultErrorMessage(err)
  } finally {
    createBusy.value = false
  }
}

// -- recents ------------------------------------------------------------------

async function forgetRecent(path: string) {
  await vault.forgetRecent(path)
}
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
        <!-- Block 1: open one of the vaults seen before. -->
        <Card>
          <template #header>
            <div class="flex items-center gap-2">
              <LockKeyhole class="size-4 shrink-0 text-muted-foreground" />
              <h2 class="text-base font-semibold">{{ t('auth.recentVaults') }}</h2>
            </div>
          </template>

          <div class="grid gap-3">
            <div v-if="vault.recents.length" class="grid max-h-64 gap-1.5 overflow-y-auto">
              <div
                v-for="recent in vault.recents"
                :key="recent.path"
                class="flex items-center gap-1 rounded-md border border-border"
              >
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
                  @click="forgetRecent(recent.path)"
                >
                  <X class="size-3.5" />
                </Button>
              </div>
            </div>

            <p v-else class="text-sm text-muted-foreground">{{ t('auth.noRecentVaults') }}</p>

            <Button variant="ghost" class="justify-start gap-2" @click="openUnlockOther">
              <FolderOpen class="size-4" />
              {{ t('auth.openOther') }}
            </Button>
          </div>
        </Card>

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

          <Button size="lg" class="w-full" @click="openCreate">
            <FolderPlus class="size-4" />
            {{ t('auth.createVaultButton') }}
          </Button>
        </Card>
      </div>
    </div>

    <!-- Unlock dialog: password for a vault chosen above, or for any file. -->
    <Dialog
      v-model:open="unlockOpen"
      :title="t('auth.unlockTitle')"
      :description="t('auth.unlockHint')"
      size="sm"
    >
      <form id="unlock-form" class="grid gap-4" @submit.prevent="submitUnlock">
        <div v-if="!unlockPathFixed" class="grid gap-1.5">
          <Label for="unlock-path">{{ t('auth.vaultPath') }}</Label>
          <div class="flex gap-1.5">
            <Input id="unlock-path" v-model="unlockTargetPath" :placeholder="t('auth.pathPlaceholder')" />
            <Button variant="outline" type="button" :aria-label="t('auth.browse')" :title="t('auth.browse')" @click="browseUnlockPath">
              <FolderOpen class="size-4" />
            </Button>
          </div>
        </div>

        <div class="grid gap-1.5">
          <Label for="unlock-password">{{ t('auth.password') }}</Label>
          <!-- biome-ignore lint/a11y/noAutofocus: the dialog exists to type the password. -->
          <Input id="unlock-password" v-model="unlockPassword" type="password" autocomplete="current-password" autofocus />
          <p class="truncate text-xs text-muted-foreground">{{ unlockPath }}</p>
        </div>

        <p
          v-if="unlockError"
          role="alert"
          class="rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive"
        >
          {{ unlockError }}
        </p>
      </form>

      <template #footer>
        <Button variant="ghost" @click="unlockOpen = false">{{ t('actions.cancel') }}</Button>
        <Button form="unlock-form" type="submit" :disabled="unlockBusy">
          <span v-if="unlockBusy" class="size-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
          {{ unlockBusy ? t('auth.unlocking') : t('auth.enter') }}
        </Button>
      </template>
    </Dialog>

    <!-- Create dialog: path + password, then the vault is saved and opened. -->
    <Dialog
      v-model:open="createOpen"
      :title="t('auth.createVaultTitle')"
      :description="t('auth.createVaultHint')"
      size="sm"
    >
      <form id="create-form" class="grid gap-4" @submit.prevent="submitCreate">
        <div class="grid gap-1.5">
          <Label for="create-path">{{ t('auth.vaultPath') }}</Label>
          <div class="flex gap-1.5">
            <Input id="create-path" v-model="createPath" :placeholder="t('auth.pathPlaceholder')" />
            <Button variant="outline" type="button" :aria-label="t('auth.browse')" :title="t('auth.browse')" @click="browseCreatePath">
              <FolderOpen class="size-4" />
            </Button>
          </div>
        </div>

        <div class="grid gap-1.5">
          <Label for="create-password">{{ t('auth.password') }}</Label>
          <!-- biome-ignore lint/a11y/noAutofocus: the dialog exists to type the password. -->
          <Input id="create-password" v-model="createPassword" type="password" autocomplete="new-password" autofocus />
        </div>

        <div class="grid gap-1.5">
          <Label for="create-password-repeat">{{ t('auth.passwordRepeat') }}</Label>
          <Input id="create-password-repeat" v-model="createPasswordRepeat" type="password" autocomplete="new-password" />
          <p v-if="passwordMatch" class="text-xs text-emerald-600">✓ {{ t('auth.passwordMatch') }}</p>
        </div>

        <p
          v-if="createError"
          role="alert"
          class="rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive"
        >
          {{ createError }}
        </p>
      </form>

      <template #footer>
        <Button variant="ghost" @click="createOpen = false">{{ t('actions.cancel') }}</Button>
        <Button form="create-form" type="submit" :disabled="createBusy">
          <span v-if="createBusy" class="size-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
          {{ createBusy ? t('auth.creating') : t('auth.createVaultButton') }}
        </Button>
      </template>
    </Dialog>

    <Dialog
      v-model:open="settingsOpen"
      :title="t('settings.appTitle')"
      :description="t('settings.appScopeHint')"
      size="lg"
    >
      <!-- Mounted per open so every visit starts from the file on disk. -->
      <AppSettingsModal v-if="settingsOpen" />
      <template #footer>
        <Button @click="settingsOpen = false">{{ t('actions.close') }}</Button>
      </template>
    </Dialog>
  </div>
</template>
