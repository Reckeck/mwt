<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useIntervalFn } from '@vueuse/core'
import { Wifi } from 'lucide-vue-next'

import { useSyncAgo, useSyncPeers, useSyncStatus } from '@/features/vault-sync'
import { formatList } from '@/shared/lib/format'

/** Who is online right now and when this device last synced with them. */
const { t } = useI18n()
const { status, connected } = useSyncStatus()
const { list } = useSyncPeers()
const ago = useSyncAgo()
const now = ref(Date.now())
useIntervalFn(() => {
  now.value = Date.now()
}, 30_000)

const text = computed(() => {
  if (!status.data.value?.enabled) return t('sync.status.off')
  if (!connected.value.length) return t('sync.status.nobody')
  const names = formatList(connected.value.map((session) => `«${session.name}»`))
  const ids = new Set(connected.value.map((session) => session.device_id))
  const last = list.value
    .filter((peer) => ids.has(peer.device_id) && peer.last_synced_at)
    .map((peer) => peer.last_synced_at as string)
    .sort()
    .at(-1)
  return last ? t('sync.status.onlineSynced', { names, when: ago(last, now.value) }) : t('sync.status.online', { names })
})
</script>

<template>
  <p class="flex items-center gap-2 rounded-md bg-muted px-3 py-2.5 text-sm" role="status">
    <Wifi class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
    <span class="min-w-0">{{ text }}</span>
  </p>
</template>
