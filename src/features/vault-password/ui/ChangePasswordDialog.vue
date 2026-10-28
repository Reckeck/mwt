<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { NewPasswordFields } from '@/shared/ui/new-password-fields'
import { firstError, minLength, required, useFormValidation } from '@/shared/lib/form'
import { MIN_VAULT_PASSWORD } from '@/shared/lib/vault-password'
import { call } from '@/shared/api/ipc'

const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()

const current = ref('')
const next = ref('')
const repeat = ref('')

/** Passwords never outlive the dialog. */
watch(open, () => {
  current.value = ''
  next.value = ''
  repeat.value = ''
})

const form = useFormValidation(
  () => ({
    current: required(current.value),
    next: firstError(required(next.value), minLength(next.value, MIN_VAULT_PASSWORD)),
    repeat: required(repeat.value) ?? (next.value === repeat.value ? null : t('vault.password.mismatch')),
  }),
  { resetOn: open },
)

const change = useMutation({
  mutationFn: () =>
    call<void>('change_password', {
      input: { current_password: current.value, new_password: next.value } satisfies ChangePasswordInput,
    }),
  onSuccess: () => {
    open.value = false
  },
  // Sync never carries the password: the other devices keep theirs.
  meta: {
    success: () => ({ text: t('vault.password.changed'), description: t('sync.passwordHint') }),
    error: 'vault.password.failed',
  },
})
</script>

<template>
  <Dialog overlay="password-change" v-model:open="open" :title="t('vault.password.title')">
    <form id="change-password-form" class="grid gap-4" @submit.prevent="form.submit(change.mutate, $event)">
      <FormField for="password-current" :label="t('vault.password.current')" required :error="form.error('current')">
        <Input id="password-current" v-model="current" type="password" autocomplete="current-password" />
      </FormField>
      <NewPasswordFields
        id="change-password"
        v-model:password="next"
        v-model:repeat="repeat"
        :label="t('vault.password.new')"
        :error="form.error('next')"
        :repeat-error="form.error('repeat')"
      />
      <p class="text-xs text-muted-foreground">{{ t('vault.password.hint') }}</p>
    </form>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button type="submit" form="change-password-form" :disabled="change.isPending.value">
        {{ t('vault.password.submit') }}
      </Button>
    </template>
  </Dialog>
</template>
