<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { MonitorSmartphone, Trash2 } from 'lucide-vue-next'

import { SYNC_PEERS_KEY, useSyncActions, useSyncStatus } from '@/features/vault-sync'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { SectionState } from '@/shared/ui/section-state'
import { SettingsSection } from '@/shared/ui/settings-section'

/** Trusted devices (forget one) and devices nearby with this vault (pair one). */
const { t } = useI18n()
const { status, offers } = useSyncStatus()
const { pair, forget } = useSyncActions()
const peers = useQuery({ queryKey: SYNC_PEERS_KEY, queryFn: () => call<SyncPeerView[]>('sync_peers') })

const enabled = computed(() => Boolean(status.data.value?.enabled))
const list = computed(() => peers.data.value ?? [])
</script>

<template>
  <SettingsSection :title="t('sync.devices.title')" :description="t('sync.devices.description')">
    <div v-if="enabled && offers.length" class="grid gap-2">
      <p class="text-sm font-medium">{{ t('sync.devices.nearby') }}</p>
      <div v-for="offer in offers" :key="offer.session_id" class="flex items-center gap-2 rounded-md border border-border px-3 py-2">
        <MonitorSmartphone class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span class="min-w-0 flex-1 truncate text-sm">{{ offer.name }}</span>
        <Button size="sm" :disabled="pair.isPending.value" @click="pair.mutate(offer.session_id)">{{ t('sync.offer.pair') }}</Button>
      </div>
    </div>

    <SectionState
      :loading="peers.isPending.value"
      :error="peers.error.value"
      :empty="!list.length"
      :empty-title="t('sync.devices.empty')"
      :empty-description="enabled ? t('sync.devices.emptyHint') : t('sync.devices.emptyOff')"
      skeleton="list"
      compact
      @retry="peers.refetch()"
    >
      <ul class="grid gap-2">
        <li v-for="peer in list" :key="peer.device_id" class="flex items-center gap-2 rounded-md border border-border px-3 py-2">
          <MonitorSmartphone class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          <div class="grid min-w-0 flex-1 gap-0.5">
            <span class="truncate text-sm font-medium">{{ peer.name }}</span>
            <span class="text-xs text-muted-foreground">
              {{ peer.last_synced_at ? t('sync.devices.lastSynced', { when: formatDateTime(peer.last_synced_at) }) : t('sync.devices.never') }}
            </span>
          </div>
          <Badge :variant="peer.online ? 'secondary' : 'outline'">{{ peer.online ? t('sync.devices.online') : t('sync.devices.offline') }}</Badge>
          <ConfirmButton
            :label="t('sync.devices.forget')"
            :confirm-label="t('sync.devices.forgetConfirm')"
            :disabled="forget.isPending.value"
            @confirm="forget.mutate(peer.device_id)"
          >
            <Trash2 class="size-3.5" />
          </ConfirmButton>
        </li>
      </ul>
      <p class="mt-2 text-xs text-muted-foreground">{{ t('sync.devices.forgetHint') }}</p>
    </SectionState>
  </SettingsSection>
</template>
