<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { LockKeyhole, ShieldCheck, FolderOpen, X, ChevronRight } from 'lucide-vue-next'

import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { parseError } from '@/lib/ipc'
import { useVaultStore } from '@/stores/vault'

const { t } = useI18n()
const router = useRouter()
const vault = useVaultStore()

const password = ref('')
const passwordRepeat = ref('')
const error = ref<string | null>(null)
const busy = ref(false)

const manualPath = ref('')
const showPathInput = ref(false)

const creating = computed(() => vault.status?.exists !== true)

onMounted(async () => {
  if (vault.status === null) {
    try {
      await vault.refreshStatus()
    } catch {
      /* keep create mode when offline/mock */
    }
  }
  await vault.loadRecents()
})

async function submit() {
  if (!vault.status) {
    error.value = String(t('auth.vaultUnavailable'))
    return
  }
  if (creating.value) {
    if (password.value.length < 4) {
      error.value = t('auth.passwordTooShort')
      return
    }
    if (password.value !== passwordRepeat.value) {
      error.value = t('auth.passwordMismatch')
      return
    }
  }
  busy.value = true
  error.value = null
  try {
    if (creating.value) {
      await vault.createVault(password.value)
    } else {
      await vault.unlock(password.value)
    }
    await router.replace({ name: 'dashboard' })
  } catch (err) {
    const maybe = err as { code?: string; message?: string }
    if (maybe.code === 'ALREADY_EXISTS') {
      await vault.refreshStatus().catch(() => {})
      error.value = t('auth.alreadyExists')
    } else if (maybe.code === 'UNSUPPORTED') {
      error.value = t('auth.unsupported')
    } else {
      error.value = maybe.code === 'SECRET' ? t('auth.wrongPassword') : maybe.message ?? String(err)
    }
  } finally {
    busy.value = false
  }
}

async function selectRecent(path: string) {
  await vault.selectVault(path)
  showPathInput.value = false
  manualPath.value = ''
}

async function useOtherPath() {
  showPathInput.value = true
  manualPath.value = vault.status?.path || ''
  error.value = null
}

async function browsePath() {
  try {
    const result = await vault.pickVaultFile(!vault.status?.exists)
    if (result?.path) {
      manualPath.value = result.path
      // Refresh status to get exists flag
      await vault.refreshStatus()
    }
  } catch (err) {
    error.value = parseError(err).message
  }
}

async function confirmPath() {
  if (!manualPath.value.trim()) {
    error.value = t('auth.pathRequired')
    return
  }
  await vault.selectVault(manualPath.value)
  showPathInput.value = false
  manualPath.value = ''
}

async function forgetRecent(path: string) {
  await vault.forgetRecent(path)
}
</script>

<template>
  <div class="flex h-full items-center justify-center bg-muted/40 p-6">
    <Card class="w-full max-w-sm">
      <template #header>
        <div class="flex flex-col items-center gap-2 text-center">
          <div class="flex size-12 items-center justify-center rounded-full bg-primary/10">
            <LockKeyhole v-if="!creating" class="size-6 text-primary" />
            <ShieldCheck v-else class="size-6 text-primary" />
          </div>
          <h2 class="text-lg font-semibold">{{ creating ? t('auth.createTitle') : t('auth.title') }}</h2>
          <p class="text-sm text-muted-foreground">{{ creating ? '' : t('auth.locked') }}</p>
        </div>
      </template>

      <form class="grid gap-4" @submit.prevent="submit">
        <!-- Step 1: Choose vault (recents or custom path) -->
        <div v-if="!showPathInput" class="grid gap-3">
          <div v-if="vault.recents.length > 0" class="grid gap-2">
            <p class="text-xs font-medium text-muted-foreground">{{ t('auth.recentVaults') }}</p>
            <div class="grid gap-1.5 max-h-40 overflow-y-auto">
              <Button
                v-for="recent in vault.recents"
                :key="recent.path"
                variant="outline"
                class="justify-start gap-2 text-left"
                :class="{ 'bg-destructive/10 text-destructive': !recent.exists }"
                @click="selectRecent(recent.path)"
              >
                <ChevronRight class="size-4 shrink-0" />
                <div class="flex-1 min-w-0">
                  <p class="truncate font-medium">{{ recent.path.split('/').pop() || recent.path }}</p>
                  <p class="truncate text-xs text-muted-foreground">{{ recent.path }}</p>
                </div>
                <div class="flex items-center gap-1.5">
                  <span v-if="recent.exists" class="text-xs text-emerald-600">{{ t('auth.exists') }}</span>
                  <span v-else class="text-xs text-destructive">{{ t('auth.missing') }}</span>
                  <Button variant="ghost" size="icon" @click.stop="forgetRecent(recent.path)">
                    <X class="size-3.5" />
                  </Button>
                </div>
              </Button>
            </div>
          </div>

          <Button
            variant="outline"
            class="justify-start gap-2"
            @click="useOtherPath"
          >
            <FolderOpen class="size-4" />
            {{ t('auth.otherPath') }}
          </Button>
        </div>

        <!-- Step 2: Custom path input -->
        <div v-else class="grid gap-3">
          <div class="grid gap-1.5">
            <Label for="vault-path">{{ t('auth.vaultPath') }}</Label>
            <div class="flex gap-1.5">
              <Input
                id="vault-path"
                v-model="manualPath"
                placeholder="Выберите или введите путь к vault"
                @keydown.enter="confirmPath"
              />
              <Button variant="outline" type="button" @click="browsePath">
                <FolderOpen class="size-4" />
              </Button>
            </div>
          </div>
          <Button
            variant="outline"
            class="justify-start"
            @click="showPathInput = false"
          >
            <ChevronRight class="size-4 mr-1" />
            {{ t('auth.backToRecents') }}
          </Button>
        </div>

        <div class="grid gap-1.5">
          <Label for="pw">{{ t('auth.password') }}</Label>
          <!-- biome-ignore lint/a11y/noAutofocus: the unlock screen exists to type the password. -->
          <Input id="pw" v-model="password" type="password" autocomplete="current-password" autofocus />
        </div>

        <div v-if="creating" class="grid gap-1.5">
          <Label for="pw2">{{ t('auth.passwordRepeat') }}</Label>
          <Input id="pw2" v-model="passwordRepeat" type="password" autocomplete="new-password" />
          <p
            v-if="passwordRepeat.length > 0 && passwordRepeat === password"
            class="flex items-center gap-1 text-xs text-emerald-600"
          >
            ✓ {{ t('auth.passwordMatch') }}
          </p>
        </div>

        <p v-if="error" role="alert" class="rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive">
          {{ error }}
        </p>

        <div v-if="vault.status" class="text-xs text-muted-foreground">
          {{ vault.status.path }}
        </div>

        <Button type="submit" :disabled="busy">
          <span v-if="busy" class="size-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
          {{ creating ? t('auth.create') : t('auth.unlock') }}
        </Button>
      </form>
    </Card>
  </div>
</template>