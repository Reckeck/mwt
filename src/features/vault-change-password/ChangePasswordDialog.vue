<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { required, useFormValidation } from '@/shared/lib/form'
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

const mismatch = computed(() => repeat.value !== '' && repeat.value !== next.value)
const form = useFormValidation(
  () => ({
    current: required(current.value),
    next: required(next.value),
    repeat: required(repeat.value) ?? (mismatch.value ? t('vault.password.mismatch') : null),
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
  <Dialog v-model:open="open" :title="t('vault.password.title')">
    <form id="change-password-form" class="grid gap-4" @submit.prevent="form.submit(change.mutate, $event)">
      <FormField for="password-current" :label="t('vault.password.current')" required :error="form.error('current')">
        <Input id="password-current" v-model="current" type="password" autocomplete="current-password" />
      </FormField>
      <FormField for="password-new" :label="t('vault.password.new')" required :error="form.error('next')">
        <Input id="password-new" v-model="next" type="password" autocomplete="new-password" />
      </FormField>
      <FormField for="password-repeat" :label="t('vault.password.repeat')" required :error="form.error('repeat')">
        <Input
          id="password-repeat"
          v-model="repeat"
          type="password"
          autocomplete="new-password"
          :aria-invalid="mismatch"
        />
        <p v-if="mismatch && !form.error('repeat')" class="text-xs text-destructive">{{ t('vault.password.mismatch') }}</p>
      </FormField>
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
