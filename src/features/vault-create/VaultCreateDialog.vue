<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Eye, EyeOff, FolderOpen } from 'lucide-vue-next'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'

import { BaseCurrencyField } from '@/features/base-currency-choice'

import { MIN_VAULT_PASSWORD, useCreateVault } from './useCreateVault'
import VaultCryptoChoice from './VaultCryptoChoice.vue'
import { useVaultStore } from '@/shared/store/vault'
import { firstError, minLength, required, useFormValidation } from '@/shared/lib/form'

const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const vault = useVaultStore()
const { path, password, passwordRepeat, passwordMatch, crypto, baseCurrency, error, busy, reset, browse, submit } =
  useCreateVault()
const { currency, asked } = baseCurrency

/** Shown as typed: a new password is easy to mistype twice the same way. */
const reveal = ref(false)

watch(open, (isOpen) => {
  if (isOpen) {
    reveal.value = false
    void reset()
  }
})

const form = useFormValidation(
  () => ({
    path: required(path.value),
    password: firstError(required(password.value), minLength(password.value, MIN_VAULT_PASSWORD)),
    repeat: required(passwordRepeat.value) ?? (password.value === passwordRepeat.value ? null : t('auth.passwordMismatch')),
    currency: asked.value ? required(currency.value) : null,
  }),
  { resetOn: open },
)
</script>

<template>
  <!-- Path + password, then the vault is saved and opened. -->
  <Dialog v-model:open="open" :title="t('auth.createVaultTitle')" :description="t(vault.appStorage ? 'auth.createVaultHintApp' : 'auth.createVaultHint')" size="sm">
    <form id="create-form" class="grid gap-4" @submit.prevent="form.submit(submit, $event)">
      <!-- App storage (Android/iOS): a name; the file lives inside the app. -->
      <FormField
        v-if="vault.appStorage"
        for="create-path"
        :label="t('auth.vaultName')"
        :hint="t('auth.vaultNameHint')"
        required
        :error="form.error('path')"
      >
        <Input id="create-path" v-model="path" :placeholder="t('auth.vaultNamePlaceholder')" />
      </FormField>
      <FormField v-else for="create-path" :label="t('auth.vaultPath')" required :error="form.error('path')">
        <div class="flex gap-1.5">
          <Input id="create-path" v-model="path" :placeholder="t('auth.pathPlaceholder')" />
          <Button variant="outline" type="button" :aria-label="t('auth.browse')" :title="t('auth.browse')" @click="browse">
            <FolderOpen class="size-4" />
          </Button>
        </div>
      </FormField>

      <FormField
        for="create-password"
        :label="t('auth.password')"
        :hint="t('auth.passwordMinHint', { count: MIN_VAULT_PASSWORD })"
        required
        :error="form.error('password')"
      >
        <div class="flex gap-1.5">
          <!-- biome-ignore lint/a11y/noAutofocus: the dialog exists to type the password. -->
          <Input id="create-password" v-model="password" :type="reveal ? 'text' : 'password'" autocomplete="new-password" autofocus />
          <Button
            variant="outline"
            size="icon"
            type="button"
            :aria-pressed="reveal"
            :aria-label="reveal ? t('auth.hidePassword') : t('auth.showPassword')"
            :title="reveal ? t('auth.hidePassword') : t('auth.showPassword')"
            @click="reveal = !reveal"
          >
            <component :is="reveal ? EyeOff : Eye" class="size-4" />
          </Button>
        </div>
      </FormField>

      <FormField for="create-password-repeat" :label="t('auth.passwordRepeat')" required :error="form.error('repeat')">
        <Input id="create-password-repeat" v-model="passwordRepeat" :type="reveal ? 'text' : 'password'" autocomplete="new-password" />
        <p v-if="passwordMatch && !form.error('repeat')" class="text-xs text-success">✓ {{ t('auth.passwordMatch') }}</p>
      </FormField>

      <p class="rounded-md border border-warning/50 bg-warning/10 px-3 py-2 text-sm">{{ t('auth.passwordNoRecovery') }}</p>

      <BaseCurrencyField v-if="asked" v-model="currency" :error="form.error('currency')" />
      <VaultCryptoChoice v-model="crypto" />

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
      <Button form="create-form" type="submit" :disabled="busy" :loading="busy">
        {{ t('auth.createVaultButton') }}
      </Button>
    </template>
  </Dialog>
</template>
