<script setup lang="ts">
import { watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { FolderOpen } from 'lucide-vue-next'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Spinner } from '@/shared/ui/spinner'

import { useUnlockVault } from './useUnlockVault'
import type { VaultUnlockDialogProps } from './vault-unlock-dialog.types'
import { required, useFormValidation } from '@/shared/lib/form'

const props = defineProps<VaultUnlockDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const { password, error, busy, pathFixed, targetPath, path, reset, browse, submit } = useUnlockVault()

const form = useFormValidation(
  () => ({ path: pathFixed.value ? null : required(path.value), password: required(password.value) }),
  { resetOn: open },
)

// Every opening starts clean, for whichever vault it was opened for.
watch(open, (isOpen) => {
  if (isOpen) reset(props.path)
})
</script>

<template>
  <!-- Password for a vault chosen from the recents, or for any file. -->
  <Dialog v-model:open="open" :title="t('auth.unlockTitle')" :description="t('auth.unlockHint')" size="sm">
    <form id="unlock-form" class="grid gap-4" @submit.prevent="form.submit(submit, $event)">
      <FormField v-if="!pathFixed" for="unlock-path" :label="t('auth.vaultPath')" required :error="form.error('path')">
        <div class="flex gap-1.5">
          <Input id="unlock-path" v-model="targetPath" :placeholder="t('auth.pathPlaceholder')" />
          <Button variant="outline" type="button" :aria-label="t('auth.browse')" :title="t('auth.browse')" @click="browse">
            <FolderOpen class="size-4" />
          </Button>
        </div>
      </FormField>

      <FormField for="unlock-password" :label="t('auth.password')" required :error="form.error('password')">
        <!-- biome-ignore lint/a11y/noAutofocus: the dialog exists to type the password. -->
        <Input id="unlock-password" v-model="password" type="password" autocomplete="current-password" autofocus />
        <p class="truncate text-xs text-muted-foreground">{{ path }}</p>
      </FormField>

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
      <Button form="unlock-form" type="submit" :disabled="busy">
        <Spinner v-if="busy" />
        {{ busy ? t('auth.unlocking') : t('auth.enter') }}
      </Button>
    </template>
  </Dialog>
</template>
