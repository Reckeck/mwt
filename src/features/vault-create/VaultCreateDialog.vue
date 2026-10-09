<script setup lang="ts">
import { watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { FolderOpen } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'

import { useCreateVault } from './useCreateVault'

const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const { path, password, passwordRepeat, passwordMatch, error, busy, reset, browse, submit } = useCreateVault()

watch(open, (isOpen) => {
  if (isOpen) void reset()
})
</script>

<template>
  <!-- Path + password, then the vault is saved and opened. -->
  <Dialog v-model:open="open" :title="t('auth.createVaultTitle')" :description="t('auth.createVaultHint')" size="sm">
    <form id="create-form" class="grid gap-4" @submit.prevent="submit">
      <div class="grid gap-1.5">
        <Label for="create-path">{{ t('auth.vaultPath') }}</Label>
        <div class="flex gap-1.5">
          <Input id="create-path" v-model="path" :placeholder="t('auth.pathPlaceholder')" />
          <Button variant="outline" type="button" :aria-label="t('auth.browse')" :title="t('auth.browse')" @click="browse">
            <FolderOpen class="size-4" />
          </Button>
        </div>
      </div>

      <div class="grid gap-1.5">
        <Label for="create-password">{{ t('auth.password') }}</Label>
        <!-- biome-ignore lint/a11y/noAutofocus: the dialog exists to type the password. -->
        <Input id="create-password" v-model="password" type="password" autocomplete="new-password" autofocus />
      </div>

      <div class="grid gap-1.5">
        <Label for="create-password-repeat">{{ t('auth.passwordRepeat') }}</Label>
        <Input id="create-password-repeat" v-model="passwordRepeat" type="password" autocomplete="new-password" />
        <p v-if="passwordMatch" class="text-xs text-emerald-600">✓ {{ t('auth.passwordMatch') }}</p>
      </div>

      <p
        v-if="error"
        role="alert"
        class="rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive"
      >
        {{ error }}
      </p>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="create-form" type="submit" :disabled="busy">
        <span v-if="busy" class="size-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
        {{ busy ? t('auth.creating') : t('auth.createVaultButton') }}
      </Button>
    </template>
  </Dialog>
</template>
