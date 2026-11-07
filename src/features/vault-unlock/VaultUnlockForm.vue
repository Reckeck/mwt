<script setup lang="ts">
import { watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { BaseCurrencyField } from '@/features/base-currency-choice'
import { BiometricUnlockButton } from '@/features/biometric-unlock'
import { required, useFormValidation } from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import SyncConflictChoice from './SyncConflictChoice.vue'
import { useUnlockVault } from './useUnlockVault'

/**
 * The password and `Open` of the unlock card (Full-New-Encrypt 3): for a vault with a
 * password a field, for a plain one nothing but the button.
 */
const props = defineProps<{
  /** The vault in the field above. */
  path: string
  /** The vault has no password: nothing to type. */
  plain: boolean
  /** Its file is gone: nothing to open. */
  disabled?: boolean
}>()

const { t } = useI18n()
const settings = useSettingsStore()
const { password, baseCurrency, error, busy, conflict, reset, submit, resolveConflict } = useUnlockVault()
const { currency, asked } = baseCurrency

const form = useFormValidation(() => ({
  // The file is gone: the button still answers, with the reason under the field.
  file: props.disabled ? t('auth.missing') : null,
  password: props.plain || props.disabled ? null : required(password.value),
  currency: asked.value ? required(currency.value) : null,
}))

// Another vault in the field starts the form over: no password, no error of the one before.
// The app settings may arrive after the form: whether to ask for the base currency is read again then.
watch(
  () => [props.path, settings.needsBaseCurrency] as const,
  ([path]) => reset(path),
  { immediate: true },
)
</script>

<template>
  <form class="grid gap-3" data-unlock-form @submit.prevent="form.submit(submit, $event)">
    <FormField v-if="!plain && !props.disabled" for="unlock-card-password" :label="t('auth.password')" :error="form.error('password')">
      <Input id="unlock-card-password" v-model="password" type="password" autocomplete="current-password" />
    </FormField>

    <BaseCurrencyField v-if="asked" v-model="currency" :error="form.error('currency')" />

    <SyncConflictChoice v-if="conflict" :busy="busy" @keep="resolveConflict" />

    <p
      v-if="error"
      role="alert"
      class="rounded-md border border-destructive/50 bg-destructive/10 px-3 py-2 text-sm text-destructive"
    >
      {{ error }}
    </p>

    <p v-if="form.error('file')" role="alert" class="text-xs text-destructive">{{ form.error('file') }}</p>

    <Button type="submit" size="lg" class="w-full" :disabled="busy" :loading="busy">
      {{ t('auth.open') }}
    </Button>

    <BiometricUnlockButton v-if="!plain && !props.disabled" :path="path" :disabled="busy" />
  </form>
</template>
