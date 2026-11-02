<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Laptop, MonitorSmartphone, Plus, Smartphone } from 'lucide-vue-next'

import { useSyncActions, useSyncAgo, useSyncPeers, useSyncStatus } from '@/features/vault-sync'
import { useIsPhone } from '@/shared/lib/phone'
import { useVaultStore } from '@/shared/store/vault'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { SectionState } from '@/shared/ui/section-state'
import { SettingsSection } from '@/shared/ui/settings-section'

/**
 * Trusted devices: this one first (marked `main` when it is), then the paired
 * ones (forget one), the forgotten ones with `Pair again`, and `Add a device`: one device nearby is paired at once (the code
 * dialog follows), several or none are listed under the button.
 */
const { t } = useI18n()
const { status, offers } = useSyncStatus()
const { peers, list, forgotten } = useSyncPeers()
const { pair, pairAgain, forget, resume } = useSyncActions()
const ago = useSyncAgo()
const isPhone = useIsPhone()
const vault = useVaultStore()

const enabled = computed(() => Boolean(status.data.value?.enabled))
const isMain = computed(() => Boolean(status.data.value?.prefer_main) || status.data.value?.role === 'main')
const adding = ref(false)
const showNearby = computed(() => adding.value || (enabled.value && offers.value.length > 1))

/** One device is waiting: this is the one to add. Otherwise let the user pick (or wait for) one. */
function add() {
  const [only] = offers.value
  if (enabled.value && offers.value.length === 1 && only) pair.mutate(only.session_id)
  else adding.value = !adding.value
}

function lastLine(peer: SyncPeerView) {
  const state = t(peer.online ? 'sync.devices.stateOn' : 'sync.devices.stateOff')
  const when = peer.last_synced_at ? ago(peer.last_synced_at, Date.now()) : t('sync.devices.neverShort')
  return `${state} · ${when}`
}
</script>

<template>
  <SettingsSection :title="t('sync.devices.title')" :description="t('sync.devices.description')">
    <SectionState :loading="peers.isPending.value || status.isPending.value" :error="peers.error.value" skeleton="list" compact @retry="peers.refetch()">
      <ul class="grid" data-tour="sync-device-list">
        <li class="flex items-center gap-3 border-b border-border py-2.5">
          <span class="grid size-9 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
            <component :is="isPhone ? Smartphone : Laptop" class="size-4" aria-hidden="true" />
          </span>
          <div class="grid min-w-0 flex-1 gap-0.5">
            <span class="truncate text-sm font-medium">{{ status.data.value?.device_name }} · {{ t('sync.devices.thisDevice') }}</span>
            <span v-if="isMain" class="text-xs text-muted-foreground">{{ t('sync.devices.main') }}</span>
          </div>
          <Badge v-if="isMain" variant="secondary">{{ t('sync.devices.main') }}</Badge>
        </li>
        <li v-for="peer in list" :key="peer.device_id" class="flex items-center gap-3 border-b border-border py-2.5 last:border-b-0">
          <span class="grid size-9 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
            <MonitorSmartphone class="size-4" aria-hidden="true" />
          </span>
          <div class="grid min-w-0 flex-1 gap-0.5">
            <span class="flex min-w-0 items-center gap-2">
              <span class="truncate text-sm font-medium">{{ peer.name }}</span>
              <Badge v-if="peer.paused" variant="warning" class="shrink-0">{{ t('sync.devices.paused') }}</Badge>
              <Badge v-if="peer.cut_by" variant="warning" class="shrink-0">
                {{ t(peer.cut_by === 'paused' ? 'sync.devices.cutPaused' : 'sync.devices.cutForgotten') }}
              </Badge>
            </span>
            <span class="text-xs text-muted-foreground">{{ lastLine(peer) }}</span>
          </div>
          <Button
            v-if="peer.paused"
            variant="outline"
            class="shrink-0"
            :disabled="resume.isPending.value"
            :loading="resume.isPending.value"
            @click="resume.mutate(peer.device_id)"
          >
            {{ t('sync.devices.resume') }}
          </Button>
          <Button
            v-if="peer.cut_by === 'forgotten'"
            variant="outline"
            class="shrink-0"
            :disabled="pairAgain.isPending.value"
            :loading="pairAgain.isPending.value"
            @click="pairAgain.mutate(peer.device_id)"
          >
            {{ t('sync.devices.pairAgain') }}
          </Button>
          <ConfirmButton
            show-label
            :label="t('sync.devices.forgetShort')"
            :confirm-label="t('sync.devices.forgetConfirm', { name: peer.name })"
            :action-label="t('sync.devices.forgetShort')"
            :disabled="forget.isPending.value"
            @confirm="forget.mutate(peer.device_id)"
          />
        </li>
      </ul>

      <div v-if="forgotten.length" class="grid gap-1" data-tour="sync-forgotten-devices">
        <p class="text-sm font-medium">{{ t('sync.devices.forgottenTitle') }}</p>
        <p class="text-xs text-muted-foreground">{{ t('sync.devices.forgottenHint') }}</p>
        <ul class="grid">
          <li v-for="peer in forgotten" :key="peer.device_id" class="flex items-center gap-3 border-b border-border py-2.5 last:border-b-0">
            <span class="grid size-9 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
              <MonitorSmartphone class="size-4" aria-hidden="true" />
            </span>
            <div class="grid min-w-0 flex-1 gap-0.5">
              <span class="flex min-w-0 items-center gap-2">
                <span class="truncate text-sm font-medium">{{ peer.name }}</span>
                <Badge variant="secondary" class="shrink-0">{{ t('sync.devices.forgottenBadge') }}</Badge>
              </span>
            </div>
            <Button
              variant="outline"
              class="shrink-0"
              :disabled="pairAgain.isPending.value"
              :loading="pairAgain.isPending.value"
              @click="pairAgain.mutate(peer.device_id)"
            >
              {{ t('sync.devices.pairAgain') }}
            </Button>
          </li>
        </ul>
      </div>

      <div v-if="showNearby" class="grid gap-2 rounded-md border border-border p-3">
        <p class="text-sm font-medium">{{ t('sync.devices.nearby') }}</p>
        <div v-for="offer in enabled ? offers : []" :key="offer.session_id" class="flex items-center gap-2">
          <MonitorSmartphone class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
          <span class="min-w-0 flex-1 truncate text-sm">{{ offer.name }}</span>
          <Button size="sm" :disabled="pair.isPending.value" @click="pair.mutate(offer.session_id)">{{ t('sync.offer.pair') }}</Button>
        </div>
        <p v-if="!enabled || !offers.length" class="text-sm text-muted-foreground">
          {{ enabled ? t('sync.devices.addHint') : t('sync.devices.emptyOff') }}
        </p>
      </div>

      <p v-if="vault.sandbox" class="text-sm text-muted-foreground" data-devices-sandbox>{{ t('errors.SANDBOX') }}</p>
      <div data-tour="sync-add-device">
        <Button class="gap-2" :disabled="vault.sandbox" @click="add">
          <Plus class="size-4" aria-hidden="true" />{{ t('sync.devices.add') }}
        </Button>
      </div>
      <p v-if="list.length" class="text-xs text-muted-foreground">{{ t('sync.devices.forgetHint') }}</p>
    </SectionState>
  </SettingsSection>
</template>
