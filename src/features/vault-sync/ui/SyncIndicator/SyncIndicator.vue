<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { RefreshCw, TriangleAlert } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Spinner } from '@/shared/ui/spinner'
import { settingsQuery } from '@/shared/lib/settings-link'

import { useSyncStatus } from '../../model/use-sync-status'

/** In the top bar while sync is on: how many devices are in sync; opens the sync settings. */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const { status, sessions, connected } = useSyncStatus()

const enabled = computed(() => Boolean(status.data.value?.enabled))
const busy = computed(() => sessions.value.some((session) => session.phase === 'exchanging' || session.phase === 'connecting'))
const unreliable = computed(() => sessions.value.some((session) => session.unreliable))
const label = computed(() => {
  const role = status.data.value?.role ?? 'off'
  if (connected.value.length) return t('sync.indicator.connected', { count: connected.value.length })
  return t(`sync.role.${role}`)
})

function openSettings() {
  void router.push({ query: settingsQuery(route.query, 'sync') })
}
</script>

<template>
  <Button
    v-if="enabled"
    variant="ghost"
    size="sm"
    class="shrink-0 gap-1.5"
    :title="label"
    data-tour="sync-indicator"
    @click="openSettings"
  >
    <Spinner v-if="busy" />
    <TriangleAlert v-else-if="unreliable" class="size-4 text-warning" />
    <RefreshCw v-else class="size-4" :class="connected.length ? 'text-success' : 'text-muted-foreground'" />
    <span class="hidden tabular-nums lg:inline">{{ label }}</span>
  </Button>
</template>
