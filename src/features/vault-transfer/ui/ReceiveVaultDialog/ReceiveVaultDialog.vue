<script setup lang="ts">
// check-ui-ignore control-swap: each phase of the transfer has its own footer action
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useIntervalFn } from '@vueuse/core'
import { CheckCircle2, Smartphone } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Spinner } from '@/shared/ui/spinner'

import { useReceiveVault } from '../../model/use-receive-vault'
import { TransferProgress } from '../TransferProgress'

/**
 * «Получить хранилище»: this device is visible nearby by its name for five
 * minutes. The other device picks it, this one accepts, then shows the code
 * to type there; only then does the vault come.
 */
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ received: [path: string] }>()
const { t } = useI18n()
const { view, phase, start, decide, stop } = useReceiveVault()

watch(
  open,
  (isOpen) => {
    if (isOpen) start.mutate()
    else stop()
  },
  { immediate: true },
)

const now = ref(Date.now())
useIntervalFn(() => {
  now.value = Date.now()
}, 1000)
const left = computed(() => {
  const expires = view.value?.expires_ms
  if (!expires) return ''
  const seconds = Math.max(0, Math.round((expires - now.value) / 1000))
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
})
const code = computed(() => {
  const raw = view.value?.code ?? ''
  return `${raw.slice(0, 3)} ${raw.slice(3)}`
})
const error = computed(() => (view.value?.error ? t(`errors.${view.value.error}`) : ''))

function openVault() {
  const path = view.value?.saved_path
  open.value = false
  if (path) emit('received', path)
}
</script>

<template>
  <Dialog v-model:open="open" size="sm" :title="t('transfer.receive.title')" :description="t('transfer.receive.description')">
    <div class="grid gap-4 py-2">
      <template v-if="phase === 'waiting' || phase === 'idle'">
        <div class="grid justify-items-center gap-2 text-center">
          <Smartphone class="size-8 text-muted-foreground" />
          <p class="text-sm text-muted-foreground">{{ t('transfer.receive.visibleAs') }}</p>
          <p class="break-words text-3xl font-semibold" data-transfer-name>{{ view?.device_name || '…' }}</p>
          <p v-if="left" class="text-xs text-muted-foreground tabular-nums">{{ t('transfer.receive.left', { time: left }) }}</p>
        </div>
        <p class="text-sm text-muted-foreground">{{ t('transfer.receive.howTo') }}</p>
        <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
      </template>

      <template v-else-if="phase === 'request'">
        <p class="text-center text-base">{{ t('transfer.receive.request', { name: view?.sender_name ?? '' }) }}</p>
        <p class="text-sm text-muted-foreground">{{ t('transfer.receive.requestHint') }}</p>
      </template>

      <template v-else-if="phase === 'code'">
        <p class="text-sm text-muted-foreground">{{ t('transfer.receive.codeHint', { name: view?.sender_name ?? '' }) }}</p>
        <p class="py-2 text-center font-mono text-4xl font-semibold tracking-widest tabular-nums" aria-live="polite">{{ code }}</p>
        <p v-if="error" class="text-sm text-destructive">{{ error }}</p>
      </template>

      <template v-else-if="phase === 'transferring'">
        <p class="text-sm">{{ t('transfer.receive.receiving', { name: view?.sender_name ?? '' }) }}</p>
        <TransferProgress :done="view?.done_bytes ?? 0" :total="view?.total_bytes ?? 0" />
      </template>

      <template v-else-if="phase === 'done'">
        <div class="grid justify-items-center gap-2 text-center">
          <CheckCircle2 class="size-8 text-success" />
          <p class="text-base font-medium">{{ t('transfer.receive.done') }}</p>
          <p class="text-sm text-muted-foreground">{{ t('transfer.receive.doneHint') }}</p>
        </div>
      </template>

      <template v-else-if="phase === 'failed'">
        <p class="text-sm text-destructive">{{ error }}</p>
      </template>
    </div>

    <template #footer>
      <template v-if="phase === 'request'">
        <Button variant="outline" :disabled="decide.isPending.value" @click="decide.mutate(false)">{{ t('transfer.receive.decline') }}</Button>
        <Button class="gap-2" :disabled="decide.isPending.value" @click="decide.mutate(true)" :loading="decide.isPending.value">
          {{ t('transfer.receive.accept') }}
        </Button>
      </template>
      <Button v-else-if="phase === 'done'" @click="openVault">{{ t('transfer.receive.open') }}</Button>
      <Button v-else-if="phase === 'failed'" @click="start.mutate()">{{ t('transfer.receive.again') }}</Button>
      <template v-else>
        <span v-if="phase === 'waiting' || phase === 'idle'" class="flex items-center gap-2 text-sm text-muted-foreground sm:mr-auto">
          <Spinner />
          {{ t('transfer.receive.waiting') }}
        </span>
        <Button variant="outline" @click="open = false">{{ t('actions.cancel') }}</Button>
      </template>
    </template>
  </Dialog>
</template>
