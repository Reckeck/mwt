<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Lock } from 'lucide-vue-next'

import { call } from '@/shared/api/ipc'
import { firstError, minLength, required, useFormValidation } from '@/shared/lib/form'
import { pluralForm } from '@/shared/lib/format'
import { MIN_VAULT_PASSWORD } from '@/shared/lib/vault-password'
import { useVaultStore } from '@/shared/store/vault'
import { Button } from '@/shared/ui/button'
import { Checkbox } from '@/shared/ui/checkbox'
import { Dialog } from '@/shared/ui/dialog'
import { Label } from '@/shared/ui/label'
import { NewPasswordFields } from '@/shared/ui/new-password-fields'

/** `Set password` for a vault without one (Full-New-Encrypt 5): the file and its external copy are encrypted at once. */
const props = defineProps<{ externalCopy: boolean }>()
const open = defineModel<boolean>('open', { required: true })

const { t, locale } = useI18n()
const vault = useVaultStore()
const queryClient = useQueryClient()

/** While the vault has no password every copy of it is plain: those are the ones `Delete old copies` takes away. */
const backups = useQuery({
  queryKey: ['vault-backups'],
  queryFn: () => call<VaultBackup[]>('list_vault_backups'),
  enabled: open,
})
const plainCount = computed(() => backups.data.value?.length ?? 0)

const password = ref('')
const repeat = ref('')
/** On by default: plain backups are the commonest leak after encryption is switched on. */
const deleteCopies = ref(true)

/** Passwords never outlive the dialog. */
watch(open, () => {
  password.value = ''
  repeat.value = ''
  deleteCopies.value = true
})

const form = useFormValidation(
  () => ({
    password: firstError(required(password.value), minLength(password.value, MIN_VAULT_PASSWORD)),
    repeat: required(repeat.value) ?? (password.value === repeat.value ? null : t('auth.passwordMismatch')),
  }),
  { resetOn: open },
)

const save = useMutation({
  mutationFn: () =>
    call<SetVaultPasswordResult>('set_vault_password', {
      input: { new_password: password.value, delete_plain_copies: deleteCopies.value } satisfies SetVaultPasswordInput,
    }),
  onSuccess: async () => {
    open.value = false
    // `Lock`, the auto-lock and the label `no password` follow the status; the backup list lost some copies.
    await vault.refreshStatus().catch(() => {})
    void queryClient.invalidateQueries({ queryKey: ['vault-backups'] })
  },
  meta: {
    success: (data) => {
      const removed = (data as SetVaultPasswordResult).removed_copies
      return {
        text: t('vaultPassword.setDone'),
        description: removed
          ? t(`vaultPassword.setRemoved.${pluralForm(removed, locale.value)}`, { count: removed })
          : undefined,
      }
    },
    error: 'vaultPassword.setFailed',
  },
})
</script>

<template>
  <Dialog overlay="password-set" v-model:open="open" :title="t('vaultPassword.setTitle')" size="sm">
    <form id="set-password-form" class="grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <NewPasswordFields
        id="set-password"
        v-model:password="password"
        v-model:repeat="repeat"
        :label="t('vaultPassword.setNew')"
        :error="form.error('password')"
        :repeat-error="form.error('repeat')"
      />
      <p class="text-sm text-muted-foreground">
        {{ t(props.externalCopy ? 'vaultPassword.setNote' : 'vaultPassword.setNoteNoCopy') }}
      </p>
      <div class="flex items-start gap-3">
        <Checkbox id="set-password-delete" v-model="deleteCopies" class="mt-0.5" />
        <div class="grid gap-0.5">
          <Label for="set-password-delete">{{ t('vaultPassword.deleteCopies') }}<template v-if="plainCount"> · {{ plainCount }}</template></Label>
          <p class="text-xs text-muted-foreground">{{ t('vaultPassword.deleteCopiesHint') }}</p>
        </div>
      </div>
    </form>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button type="submit" form="set-password-form" :disabled="save.isPending.value" :loading="save.isPending.value">
        <Lock class="size-4" aria-hidden="true" />{{ t('vaultPassword.setSubmit') }}
      </Button>
    </template>
  </Dialog>
</template>
