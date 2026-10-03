<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { LockKeyhole, ShieldCheck } from 'lucide-vue-next'

import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { useVaultStore } from '@/stores/vault'

const { t } = useI18n()
const router = useRouter()
const vault = useVaultStore()

const password = ref('')
const passwordRepeat = ref('')
const error = ref<string | null>(null)
const busy = ref(false)

const creating = computed(() => vault.status?.exists !== true)

onMounted(async () => {
  if (vault.status === null) {
    try {
      await vault.refreshStatus()
    } catch {
      /* keep create mode when offline/mock */
    }
  }
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
  } catch (e) {
    const maybe = e as { code?: string; message?: string }
    if (maybe.code === 'ALREADY_EXISTS') {
      await vault.refreshStatus().catch(() => {})
      error.value = t('auth.alreadyExists')
    } else {
      error.value = maybe.code === 'SECRET' ? t('auth.wrongPassword') : maybe.message ?? String(e)
    }
  } finally {
    busy.value = false
  }
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
        <div class="grid gap-1.5">
          <Label for="pw">{{ t('auth.password') }}</Label>
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