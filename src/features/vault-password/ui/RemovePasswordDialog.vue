<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { required, useFormValidation } from '@/shared/lib/form'
import { useVaultStore } from '@/shared/store/vault'
import { Button } from '@/shared/ui/button'
import { Checkbox } from '@/shared/ui/checkbox'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'

/**
 * `Remove password` (Full-New-Encrypt 4): the current password proves it is the owner,
 * the question says what becomes readable. A vault with an external copy needs the consent
 * that the copy loses its password too (`external_copy_too`).
 */
const props = defineProps<{
  /** Name of the external copy, `null` when the vault has none. */
  externalCopy: string | null
}>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const vault = useVaultStore()

const current = ref('')
const copyAgreed = ref(false)

/** The password never outlives the dialog. */
watch(open, () => {
  current.value = ''
  copyAgreed.value = false
})

const form = useFormValidation(
  () => ({
    current: required(current.value),
    copy: props.externalCopy !== null && !copyAgreed.value ? t('vaultPassword.removeCopyAgreeError') : null,
  }),
  { resetOn: open },
)

const remove = useMutation({
  mutationFn: () =>
    call<void>('remove_vault_password', {
      input: {
        current_password: current.value,
        external_copy_too: props.externalCopy !== null && copyAgreed.value,
      } satisfies RemoveVaultPasswordInput,
    }),
  onSuccess: async () => {
    open.value = false
    await vault.refreshStatus().catch(() => {})
  },
  meta: { success: 'vaultPassword.removeDone', error: 'vaultPassword.removeFailed' },
})
</script>

<template>
  <Dialog overlay="password-remove" v-model:open="open" :title="t('vaultPassword.removeTitle')" size="sm">
    <form id="remove-password-form" class="grid gap-4" @submit.prevent="form.submit(remove.mutate, $event)">
      <FormField for="remove-password-current" :label="t('vaultPassword.removeCurrent')" required :error="form.error('current')">
        <!-- biome-ignore lint/a11y/noAutofocus: the password is the only thing to type here. -->
        <Input id="remove-password-current" v-model="current" type="password" autocomplete="current-password" autofocus />
      </FormField>
      <p class="rounded-md border border-warning/50 bg-warning/10 px-3 py-2 text-sm" data-remove-warning>
        {{ props.externalCopy ? t('vaultPassword.removeWarnCopy', { name: props.externalCopy }) : t('vaultPassword.removeWarn') }}
      </p>
      <div v-if="props.externalCopy" class="grid gap-1.5">
        <div class="flex items-start gap-3">
          <Checkbox id="remove-password-copy" v-model="copyAgreed" class="mt-0.5" :aria-invalid="!!form.error('copy')" />
          <div class="grid gap-0.5">
            <Label for="remove-password-copy">{{ t('vaultPassword.removeCopyAgree') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('vaultPassword.removeCopyAgreeHint') }}</p>
          </div>
        </div>
        <p v-if="form.error('copy')" class="text-xs text-destructive">{{ form.error('copy') }}</p>
      </div>
      <p class="text-sm text-muted-foreground">{{ t('vaultPassword.removeBackups') }}</p>
    </form>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button
        type="submit"
        form="remove-password-form"
        variant="destructive"
        :disabled="remove.isPending.value"
        :loading="remove.isPending.value"
      >
        {{ t('vaultPassword.removeSubmit') }}
      </Button>
    </template>
  </Dialog>
</template>
