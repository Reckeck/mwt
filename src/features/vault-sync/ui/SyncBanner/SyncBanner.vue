<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { MonitorSmartphone } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'

import { useSyncActions } from '../../model/use-sync-actions'
import { useSyncStatus } from '../../model/use-sync-status'

/**
 * A device nearby has this vault open and is not paired yet. Nothing happens
 * until the user presses `Pair` — a vault never starts syncing with a
 * device on its own.
 */
const { t } = useI18n()
const { offers } = useSyncStatus()
const { pair, dismiss } = useSyncActions()
const dismissed = ref(new Set<string>())

const shown = computed(() => offers.value.filter((offer) => !dismissed.value.has(offer.session_id)))

/** `Not now`: hidden at once; the backend stops offering this device until sync restarts. */
function later(sessionId: string) {
  dismissed.value = new Set([...dismissed.value, sessionId])
  dismiss.mutate(sessionId)
}
</script>

<template>
  <!-- Floats over the page under the header: an offer arriving must not push the page down (rule 17). -->
  <div v-if="shown.length" class="sync-banner pointer-events-none fixed inset-x-0 z-banner flex justify-center px-4">
    <div class="grid w-full max-w-md gap-2">
      <Alert v-for="offer in shown" :key="offer.session_id" class="pointer-events-auto bg-card shadow-lg">
        <MonitorSmartphone />
        <AlertTitle>{{ t('sync.offer.title', { name: offer.name }) }}</AlertTitle>
        <AlertDescription>
          <p>{{ t('sync.offer.body') }}</p>
          <div class="mt-2 flex flex-wrap gap-2">
            <Button
              size="sm"
              class="gap-2"
              :disabled="pair.isPending.value"
              :loading="pair.isPending.value && pair.variables.value === offer.session_id"
              @click="pair.mutate(offer.session_id)"
            >
              {{ t('sync.offer.pair') }}
            </Button>
            <Button size="sm" variant="ghost" @click="later(offer.session_id)">{{ t('sync.offer.later') }}</Button>
          </div>
        </AlertDescription>
      </Alert>
    </div>
  </div>
</template>

<style scoped>
/* Under the header (3.5rem + the status bar); the header's height lives in AppHeader. */
.sync-banner {
  top: calc(var(--safe-top) + 4rem);
}
</style>
