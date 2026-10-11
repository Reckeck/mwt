<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call } from '@/shared/api/ipc'
import { vaultErrorMessage } from '@/shared/lib/vault-error'

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
const valid = computed(() => current.value !== '' && next.value !== '' && next.value === repeat.value)

const change = useMutation({
  mutationFn: () =>
    call<void>('change_password', {
      input: { current_password: current.value, new_password: next.value } satisfies ChangePasswordInput,
    }),
  onSuccess: () => {
    toast.success(t('vault.password.changed'))
    open.value = false
  },
  onError: (err) => toast.error(t('vault.password.failed'), { description: vaultErrorMessage(err, t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="t('vault.password.title')">
    <form id="change-password-form" class="grid gap-4" @submit.prevent="valid && change.mutate()">
      <div class="grid gap-1.5">
        <Label for="password-current">{{ t('vault.password.current') }}</Label>
        <Input id="password-current" v-model="current" type="password" autocomplete="current-password" />
      </div>
      <div class="grid gap-1.5">
        <Label for="password-new">{{ t('vault.password.new') }}</Label>
        <Input id="password-new" v-model="next" type="password" autocomplete="new-password" />
      </div>
      <div class="grid gap-1.5">
        <Label for="password-repeat">{{ t('vault.password.repeat') }}</Label>
        <Input
          id="password-repeat"
          v-model="repeat"
          type="password"
          autocomplete="new-password"
          :aria-invalid="mismatch"
        />
        <p v-if="mismatch" class="text-xs text-destructive">{{ t('vault.password.mismatch') }}</p>
      </div>
      <p class="text-xs text-muted-foreground">{{ t('vault.password.hint') }}</p>
    </form>
    <template #footer>
      <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button type="submit" form="change-password-form" :disabled="!valid || change.isPending.value">
        {{ t('vault.password.submit') }}
      </Button>
    </template>
  </Dialog>
</template>
