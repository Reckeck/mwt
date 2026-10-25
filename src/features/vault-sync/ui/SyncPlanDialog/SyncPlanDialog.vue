<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute } from 'vue-router'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import { useSyncActions } from '../../model/use-sync-actions'
import { useSyncStatus } from '../../model/use-sync-status'
import { SyncChanges } from '../SyncChanges'

/**
 * What a batch from another device will change here, before it is applied:
 * after pairing, with «Спрашивать перед синхронизацией», or while the other
 * device's clock is not trusted. Closing the window declines the batch. Not on
 * Настройки › Синхронизация › История: the same batch is answered there, in the list.
 */
const { t } = useI18n()
const { plan } = useSyncStatus()
const { decidePlan } = useSyncActions()
const route = useRoute()
const answered = ref<string | null>(null)
const answeredInList = computed(() => route.query.section === 'sync-history')

const session = computed(() => plan.value)
const open = computed({
  get: () => Boolean(session.value?.plan) && !answeredInList.value && answered.value !== session.value?.session_id,
  set: (value: boolean) => {
    if (!value && session.value) decide(false)
  },
})

watch(session, (next) => {
  if (!next) answered.value = null
})

function decide(accept: boolean) {
  const current = session.value
  if (!current) return
  answered.value = current.session_id
  decidePlan.mutate({ session_id: current.session_id, accept })
}
</script>

<template>
  <Dialog overlay="sync-plan"
    v-model:open="open"
    size="lg"
    :title="t('sync.plan.title', { name: session?.name ?? '' })"
    :description="session?.plan ? t(`sync.plan.reason.${session.plan.reason}`) : undefined"
  >
    <div v-if="session?.plan" class="grid gap-3">
      <p class="text-sm">{{ t('sync.plan.total', { count: session.plan.summary.total }) }}</p>
      <SyncChanges :summary="session.plan.summary" />
      <p class="text-xs text-muted-foreground">{{ t('sync.plan.safety') }}</p>
    </div>
    <template #footer>
      <Button variant="outline" @click="decide(false)">{{ t('sync.plan.decline') }}</Button>
      <Button class="gap-2" :disabled="decidePlan.isPending.value" @click="decide(true)" :loading="decidePlan.isPending.value">
        {{ t('sync.plan.apply') }}
      </Button>
    </template>
  </Dialog>
</template>
