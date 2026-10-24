<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { FolderOpen } from 'lucide-vue-next'

import { BaseCurrencyField } from '@/features/base-currency-choice'
import { firstError, minLength, required, useFormValidation } from '@/shared/lib/form'
import { MIN_VAULT_PASSWORD } from '@/shared/lib/vault-password'
import { useVaultStore } from '@/shared/store/vault'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { NewPasswordFields } from '@/shared/ui/new-password-fields'
import { WizardSteps } from '@/shared/ui/wizard-steps'

import { useCreateVault } from './useCreateVault'
import VaultCryptoChoice from './VaultCryptoChoice.vue'
import VaultProtectionChoice from './VaultProtectionChoice.vue'

const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const vault = useVaultStore()
const { path, password, passwordRepeat, withPassword, crypto, baseCurrency, error, busy, reset, browse, submit } =
  useCreateVault()
const { currency, asked } = baseCurrency

/** 1 — «Файл и защита», 2 — «Как будете вести учёт» (Full-New-Encrypt 1–2). */
const step = ref(1)
const titles = computed(() => [t('vaultCreate.stepFile'), t('vaultCreate.stepAccounting')])

watch(open, (isOpen) => {
  if (isOpen) {
    step.value = 1
    void reset()
  }
})

const form = useFormValidation(
  () =>
    step.value === 1
      ? {
          path: required(path.value),
          password: withPassword.value
            ? firstError(required(password.value), minLength(password.value, MIN_VAULT_PASSWORD))
            : null,
          repeat: withPassword.value
            ? (required(passwordRepeat.value) ?? (password.value === passwordRepeat.value ? null : t('auth.passwordMismatch')))
            : null,
        }
      : { currency: asked.value ? required(currency.value) : null },
  { resetOn: [open, step] },
)

/** «Далее» checks the first step, «Создать» the second. */
function onSubmit(event: Event) {
  if (step.value === 1) form.submit(() => (step.value = 2), event)
  else form.submit(submit, event)
}
</script>

<template>
  <!-- Where the file lies and whether it has a password, then how the books are kept; the vault is saved and opened. -->
  <Dialog v-model:open="open" :title="t('auth.createVaultTitle')" size="sm">
    <form id="create-form" class="grid gap-4" @submit.prevent="onSubmit">
      <WizardSteps :step="step" :titles="titles" />

      <template v-if="step === 1">
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

        <VaultProtectionChoice v-model="withPassword" />

        <NewPasswordFields
          v-if="withPassword"
          id="create-password"
          v-model:password="password"
          v-model:repeat="passwordRepeat"
          :error="form.error('password')"
          :repeat-error="form.error('repeat')"
        />
        <p v-else class="rounded-md border border-warning/50 bg-warning/10 px-3 py-2 text-sm" data-plain-warning>
          {{ t('vaultCreate.plainWarning') }}
        </p>
      </template>

      <template v-else>
        <BaseCurrencyField v-if="asked" v-model="currency" :error="form.error('currency')" />
        <VaultCryptoChoice v-model="crypto" />
      </template>

      <p
        v-if="error"
        role="alert"
        class="rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive"
      >
        {{ error }}
      </p>
    </form>

    <template #footer>
      <Button variant="ghost" @click="step === 1 ? (open = false) : (step = 1)">
        {{ step === 1 ? t('actions.cancel') : t('actions.back') }}
      </Button>
      <Button form="create-form" type="submit" :disabled="busy" :loading="busy">
        {{ step === 1 ? t('actions.next') : t('auth.createVaultButton') }}
      </Button>
    </template>
  </Dialog>
</template>
