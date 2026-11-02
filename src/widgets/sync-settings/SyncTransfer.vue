<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Send, Smartphone } from 'lucide-vue-next'

import { ReceiveVaultDialog, SendVaultDialog } from '@/features/vault-transfer'
import { useOpenReceivedVault } from '@/features/vault-unlock'
import { useVaultStore } from '@/shared/store/vault'
import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'

/** Hand this vault to a new device, or take another vault from one. */
const { t } = useI18n()
const sendOpen = ref(false)
const receiveOpen = ref(false)
// The sandbox keeps off the network: both buttons wait, and the note says why.
const { switchTo } = useOpenReceivedVault()
// Open the received vault: switch to it, asking for the password only when it has one.
function onReceived(path: string) {
  void switchTo(path)
}
const sandbox = computed(() => useVaultStore().sandbox)
</script>

<template>
  <SettingsSection :title="t('transfer.section.title')" :description="t('transfer.section.description')" data-tour="sync-transfer">
    <div class="grid gap-3">
      <p v-if="sandbox" class="text-sm text-muted-foreground" data-transfer-sandbox>{{ t('errors.SANDBOX') }}</p>
      <div class="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border px-3 py-2">
        <div class="grid min-w-0 flex-1 gap-0.5">
          <span class="text-sm font-medium">{{ t('transfer.section.send') }}</span>
          <span class="text-xs text-muted-foreground">{{ t('transfer.section.sendHint') }}</span>
        </div>
        <Button size="sm" class="gap-1.5" :disabled="sandbox" @click="sendOpen = true">
          <Send class="size-3.5" />
          {{ t('transfer.section.sendButton') }}
        </Button>
      </div>
      <div class="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border px-3 py-2">
        <div class="grid min-w-0 flex-1 gap-0.5">
          <span class="text-sm font-medium">{{ t('transfer.section.receive') }}</span>
          <span class="text-xs text-muted-foreground">{{ t('transfer.section.receiveHint') }}</span>
        </div>
        <Button size="sm" variant="outline" class="gap-1.5" :disabled="sandbox" @click="receiveOpen = true">
          <Smartphone class="size-3.5" />
          {{ t('transfer.section.receiveButton') }}
        </Button>
      </div>
    </div>
    <SendVaultDialog v-model:open="sendOpen" />
    <ReceiveVaultDialog v-model:open="receiveOpen" @received="onReceived" />
  </SettingsSection>
</template>
