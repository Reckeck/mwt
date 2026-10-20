<script setup lang="ts">
// check-ui-ignore control-swap: each phase of the transfer has its own footer action
import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { CheckCircle2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Spinner } from '@/shared/ui/spinner'

import { useSendVault } from '../../model/use-send-vault'
import { TransferProgress } from '../TransferProgress'
import SendVaultCode from './SendVaultCode.vue'
import SendVaultPick from './SendVaultPick.vue'

/**
 * «Передать на новое устройство»: pick the device waiting nearby, type the
 * vault password again, then the code its screen shows. Every transfer is
 * recorded and announced on all linked devices.
 */
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()
const { nearby, view, phase, browse, close, send, sendCode } = useSendVault()

watch(
  open,
  (isOpen) => {
    if (isOpen) void browse()
    else {
      close()
      send.reset()
    }
  },
  { immediate: true },
)

const error = computed(() => (view.value?.error ? t(`errors.${view.value.error}`) : ''))

function retry() {
  close()
  send.reset()
  void browse()
}
</script>

<template>
  <Dialog v-model:open="open" size="sm" :title="t('transfer.send.title')" :description="t('transfer.send.description')">
    <div class="py-2">
      <SendVaultPick
        v-if="phase === 'idle'"
        :nearby="nearby"
        :pending="send.isPending.value"
        :failure="send.error.value"
        @submit="send.mutate"
      />

      <div v-else-if="phase === 'waiting'" class="flex items-center gap-3 text-sm">
        <Spinner />
        {{ t('transfer.send.waiting', { name: view?.device_name ?? '' }) }}
      </div>

      <SendVaultCode v-else-if="phase === 'code' && view" :view="view" @submit="sendCode.mutate" />

      <div v-else-if="phase === 'transferring'" class="grid gap-3">
        <p class="text-sm">{{ t('transfer.send.sending', { name: view?.device_name ?? '' }) }}</p>
        <TransferProgress :done="view?.done_bytes ?? 0" :total="view?.total_bytes ?? 0" />
      </div>

      <div v-else-if="phase === 'done'" class="grid justify-items-center gap-2 text-center">
        <CheckCircle2 class="size-8 text-success" />
        <p class="text-base font-medium">{{ t('transfer.send.done', { name: view?.device_name ?? '' }) }}</p>
        <p class="text-sm text-muted-foreground">{{ t('transfer.send.doneHint') }}</p>
      </div>

      <p v-else-if="phase === 'failed'" class="text-sm text-destructive">{{ error }}</p>
    </div>

    <template #footer>
      <Button v-if="phase === 'idle'" type="submit" form="transfer-send-form" class="gap-2" :disabled="send.isPending.value" :loading="send.isPending.value">
        {{ t('transfer.send.submit') }}
      </Button>
      <Button v-else-if="phase === 'code'" type="submit" form="transfer-code-form" class="gap-2" :disabled="sendCode.isPending.value" :loading="sendCode.isPending.value">
        {{ t('transfer.send.confirm') }}
      </Button>
      <Button v-else-if="phase === 'failed'" @click="retry">{{ t('transfer.send.again') }}</Button>
      <Button v-if="phase === 'done'" @click="open = false">{{ t('transfer.send.close') }}</Button>
      <Button v-else variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
    </template>
  </Dialog>
</template>
