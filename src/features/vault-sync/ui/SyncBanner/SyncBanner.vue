<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { MonitorSmartphone } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'
import { Spinner } from '@/shared/ui/spinner'

import { useSyncActions } from '../../model/use-sync-actions'
import { useSyncStatus } from '../../model/use-sync-status'

/**
 * A device nearby has this vault open and is not paired yet. Nothing happens
 * until the user presses «Сопрячь» — a vault never starts syncing with a
 * device on its own.
 */
const { t } = useI18n()
const { offers } = useSyncStatus()
const { pair, dismiss } = useSyncActions()
const dismissed = ref(new Set<string>())

const shown = computed(() => offers.value.filter((offer) => !dismissed.value.has(offer.session_id)))

/** «Не сейчас»: hidden at once; the backend stops offering this device until sync restarts. */
function later(sessionId: string) {
  dismissed.value = new Set([...dismissed.value, sessionId])
  dismiss.mutate(sessionId)
}
</script>

<template>
  <div v-if="shown.length" class="mb-3 grid gap-2">
    <Alert v-for="offer in shown" :key="offer.session_id">
      <MonitorSmartphone />
      <AlertTitle>{{ t('sync.offer.title', { name: offer.name }) }}</AlertTitle>
      <AlertDescription>
        <p>{{ t('sync.offer.body') }}</p>
        <div class="mt-2 flex flex-wrap gap-2">
          <Button size="sm" class="gap-2" :disabled="pair.isPending.value" @click="pair.mutate(offer.session_id)">
            <Spinner v-if="pair.isPending.value && pair.variables.value === offer.session_id" />
            {{ t('sync.offer.pair') }}
          </Button>
          <Button size="sm" variant="ghost" @click="later(offer.session_id)">{{ t('sync.offer.later') }}</Button>
        </div>
      </AlertDescription>
    </Alert>
  </div>
</template>
