<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

/** Why a new token waits (mockup 05): anyone can send a token to an address, so it does not count until the user says so. */
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()
</script>

<template>
  <Dialog v-model:open="open" overlay="crypto-quarantine-why" size="confirm" :title="t('cryptoTrack.quarantine.whyTitle')">
    <div class="flex flex-col gap-3 text-sm">
      <p>{{ t('cryptoTrack.quarantine.whyAnyone') }}</p>
      <p>{{ t('cryptoTrack.quarantine.whyFirst') }}</p>
      <p>{{ t('cryptoTrack.quarantine.whyAfter') }}</p>
      <p class="rounded-lg bg-muted p-3 text-muted-foreground">{{ t('cryptoTrack.quarantine.whyNotProtection') }}</p>
    </div>
    <template #footer>
      <Button @click="open = false">{{ t('cryptoTrack.quarantine.gotIt') }}</Button>
    </template>
  </Dialog>
</template>
