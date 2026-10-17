<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Spinner } from '@/shared/ui/spinner'

import { useSyncActions } from '../../model/use-sync-actions'
import { useSyncStatus } from '../../model/use-sync-status'

/**
 * Both screens show a six-digit code from the encrypted handshake. The same
 * code on both means nobody sits in between; the device is trusted only when
 * the user confirms it on both.
 */
const { t } = useI18n()
const { pairing } = useSyncStatus()
const { confirmPairing } = useSyncActions()
const answered = ref<string | null>(null)

const session = computed(() => pairing.value)
const open = computed({
  get: () => Boolean(session.value) && answered.value !== session.value?.session_id,
  set: (value: boolean) => {
    if (!value && session.value) decide(false)
  },
})
const code = computed(() => {
  const raw = session.value?.pairing_code ?? ''
  return `${raw.slice(0, 3)} ${raw.slice(3)}`
})

watch(session, (next) => {
  if (!next) answered.value = null
})

function decide(accept: boolean) {
  const current = session.value
  if (!current) return
  answered.value = current.session_id
  confirmPairing.mutate({ session_id: current.session_id, accept })
}
</script>

<template>
  <Dialog v-model:open="open" size="sm" :title="t('sync.pairing.title')" :description="t('sync.pairing.description', { name: session?.name ?? '' })">
    <p class="py-4 text-center font-mono text-4xl font-semibold tracking-widest tabular-nums" aria-live="polite">{{ code }}</p>
    <p class="text-sm text-muted-foreground">{{ t('sync.pairing.hint') }}</p>
    <template #footer>
      <Button variant="outline" @click="decide(false)">{{ t('sync.pairing.differ') }}</Button>
      <Button class="gap-2" :disabled="confirmPairing.isPending.value" @click="decide(true)">
        <Spinner v-if="confirmPairing.isPending.value" />
        {{ t('sync.pairing.match') }}
      </Button>
    </template>
  </Dialog>
</template>
