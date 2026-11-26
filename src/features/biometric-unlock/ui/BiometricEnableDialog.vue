<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { required, useFormValidation } from '@/shared/lib/form'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import { useBiometricRefresh } from '../model/use-biometric'

/** Turning quick sign-in on asks for the password: it is what gets bound to the device (Security 1 · 10). */
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()
const refresh = useBiometricRefresh()

const password = ref('')
const form = useFormValidation(() => ({ password: required(password.value) }), { resetOn: open })
watch(open, (isOpen) => {
  if (isOpen) password.value = ''
})

const enable = useMutation({
  mutationFn: () => call<BiometricState>('biometric_enable', { input: { password: password.value } }),
  onSuccess: () => {
    open.value = false
    refresh()
  },
  meta: { success: 'biometric.enabled', error: 'biometric.enableFailed' },
})
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="biometric-enable"
    form-id="biometric-enable-form"
    size="sm"
    :title="t('biometric.enableTitle')"
    :description="t('biometric.enableHint')"
    :submit-label="t('biometric.enable')"
    :pending="enable.isPending.value"
    @submit="form.submit(enable.mutate, $event)"
  >
    <FormField
      for="biometric-password"
      :label="t('biometric.password')"
      required
      :hint="t('biometric.passwordHint')"
      :error="form.error('password')"
    >
      <Input id="biometric-password" v-model="password" type="password" autocomplete="current-password" />
    </FormField>
  </FormDialog>
</template>
