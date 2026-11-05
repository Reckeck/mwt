<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Smartphone, Wifi } from 'lucide-vue-next'

import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import { useSyncActions } from '../../model/use-sync-actions'
import { usePairingCountdown } from '../../model/use-pairing-countdown'
import { useSyncStatus } from '../../model/use-sync-status'

/**
 * `Add a device`: both screens show the same six-digit code from the
 * encrypted handshake, big, with the time left. The same code on both means
 * nobody sits in between; the device is trusted only when the user presses
 * `Done` here and the other device confirms too.
 */
const { t } = useI18n()
const { pairing } = useSyncStatus()
const { confirmPairing } = useSyncActions()
const isPhone = useIsPhone()
/** What was answered for a session: a decline hides the dialog, an accept keeps it as the waiting state. */
const answered = ref<{ id: string; accept: boolean } | null>(null)

const session = computed(() => pairing.value)
const open = computed({
  get: () => Boolean(session.value) && !(answered.value && !answered.value.accept && answered.value.id === session.value?.session_id),
  set: (value: boolean) => {
    if (!value && session.value) decide(false)
  },
})
/** Confirmed here, the other device has not yet. */
const waiting = computed(() => Boolean(answered.value?.accept) && answered.value?.id === session.value?.session_id)
const code = computed(() => {
  const raw = session.value?.pairing_code ?? ''
  return `${raw.slice(0, 3)} ${raw.slice(3)}`
})
const left = usePairingCountdown(computed(() => session.value?.pairing_expires_ms))

watch(session, (next) => {
  if (!next) answered.value = null
})

function reopen(sessionId: string) {
  if (answered.value?.id === sessionId) answered.value = null
}

function decide(accept: boolean) {
  const current = session.value
  if (!current) return
  answered.value = { id: current.session_id, accept }
  // Pending keeps the dialog hidden (no flicker); a refusal brings it back to try again.
  confirmPairing.mutate({ session_id: current.session_id, accept }, { onError: () => reopen(current.session_id) })
}
</script>

<template>
  <Dialog overlay="sync-pairing" v-model:open="open" size="sm" :title="t('sync.pairing.title')">
    <div class="flex flex-col items-center gap-2 py-3">
      <Smartphone class="size-8" aria-hidden="true" />
      <p class="text-center text-sm text-muted-foreground">{{ waiting ? t('sync.pairing.waiting') : t('sync.pairing.instruction') }}</p>
      <p class="text-4xl font-semibold tracking-[0.2em] tabular-nums" aria-live="polite">{{ code }}</p>
      <p v-if="left" class="text-xs text-muted-foreground">{{ t('sync.pairing.left', { time: left }) }}</p>
    </div>
    <div class="flex items-start gap-2 rounded-md bg-muted px-3 py-2.5 text-sm">
      <Wifi class="mt-0.5 size-4 shrink-0" aria-hidden="true" />
      <p class="min-w-0 flex-1">{{ t('sync.pairing.found', { name: session?.name ?? '' }) }}</p>
    </div>
    <template #footer>
      <Button v-if="!isPhone" variant="outline" @click="decide(false)">{{ t('actions.cancel') }}</Button>
      <Button :class="isPhone ? 'h-12 flex-1' : ''" :disabled="confirmPairing.isPending.value || waiting" :loading="confirmPairing.isPending.value" @click="decide(true)">
        {{ t('ui.done') }}
      </Button>
    </template>
  </Dialog>
</template>
