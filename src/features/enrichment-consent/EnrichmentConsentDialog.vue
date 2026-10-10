<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const settings = useSettingsStore()

/** Either answer is remembered, so the question is asked once. */
async function answer(consent: boolean) {
  try {
    await settings.updateApp({ enrichment_opt_in: consent })
    open.value = false
    if (consent) toast.success(t('enrichment.consent.on'))
  } catch (err) {
    toast.error(t('settings.saveFailed'), { description: codeErrorMessage(parseError(err), t) })
  }
}
</script>

<template>
  <Dialog v-model:open="open" :title="t('enrichment.consent.title')">
    <div class="grid gap-3 text-sm">
      <p>{{ t('enrichment.consent.what') }}</p>
      <ul class="grid list-disc gap-1 pl-5 text-muted-foreground">
        <li>{{ t('enrichment.consent.sent') }}</li>
        <li>{{ t('enrichment.consent.services') }}</li>
        <li>{{ t('enrichment.consent.never') }}</li>
      </ul>
      <p class="text-xs text-muted-foreground">{{ t('enrichment.consent.change') }}</p>
    </div>
    <template #footer>
      <Button variant="ghost" @click="answer(false)">{{ t('enrichment.consent.decline') }}</Button>
      <Button @click="answer(true)">{{ t('enrichment.consent.accept') }}</Button>
    </template>
  </Dialog>
</template>
