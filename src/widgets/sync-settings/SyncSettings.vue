<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { History } from 'lucide-vue-next'

import { useSyncActions, useSyncStatus } from '@/features/vault-sync'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { FormField } from '@/shared/ui/form-field'
import { useVaultStore } from '@/shared/store/vault'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { SectionState } from '@/shared/ui/section-state'
import { Select, SelectItem } from '@/shared/ui/select'
import { SettingsSection } from '@/shared/ui/settings-section'

import SyncNetwork from './SyncNetwork.vue'
import SyncStatusLine from './SyncStatusLine.vue'
import { SwitchRow } from '@/shared/ui/switch-row'

/** This device's sync switches: on/off, its name, «спрашивать», «главное». */
const { t } = useI18n()
const vault = useVaultStore()
const { status } = useSyncStatus()
const { settings, restore } = useSyncActions()

const current = computed(() => status.data.value)
const name = ref('')
watch(
  () => current.value?.device_name,
  (value) => {
    name.value = value ?? ''
  },
  { immediate: true },
)

function saveName() {
  if (name.value.trim() === (current.value?.device_name ?? '')) return
  settings.mutate({ device_name: name.value.trim() })
}

/** Android: minutes in the background before the vault locks itself (0 = never). */
const LOCK_OPTIONS = [5, 15, 30, 60, 0] as const
function lockLabel(minutes: number) {
  return minutes === 0 ? t('sync.backgroundLock.never') : t('sync.backgroundLock.minutes', { count: minutes })
}

const keyHint = computed(() => {
  const storage = current.value?.key_storage
  return storage ? t(`sync.key.${storage}`) : ''
})
</script>

<template>
  <div class="grid gap-4">
  <SettingsSection :title="t('sync.title')" :description="t('sync.description')">
    <SectionState :loading="status.isPending.value" :error="status.error.value" skeleton="list" compact @retry="status.refetch()">
      <div class="grid gap-4">
        <!-- The sandbox never syncs: the switch waits and the note says why. -->
        <p v-if="vault.sandbox" class="text-sm text-muted-foreground" data-sync-sandbox>{{ t('errors.SANDBOX') }}</p>
        <SwitchRow
          plain
          id="sync-enabled"
          :label="t('sync.enabled')"
          :hint="t('sync.enabledHint')"
          :model-value="current?.enabled ?? false"
          :disabled="settings.isPending.value || vault.sandbox"
          @update:model-value="(value: boolean) => settings.mutate({ enabled: value })"
        />

        <FormField :label="t('sync.deviceName')" for="sync-device-name" :hint="t('sync.deviceNameHint')">
          <Input id="sync-device-name" v-model="name" maxlength="40" :placeholder="current?.device_name" @blur="saveName" @keydown.enter.prevent="saveName" />
        </FormField>

        <SwitchRow
          plain
          id="sync-confirm"
          :label="t('sync.confirmFirst')"
          :hint="t('sync.confirmFirstHint')"
          :model-value="current?.confirm_first ?? false"
          @update:model-value="(value: boolean) => settings.mutate({ confirm_first: value })"
        />

        <SwitchRow
          plain
          id="sync-main"
          :label="t('sync.preferMain')"
          :hint="t('sync.preferMainHint')"
          :model-value="current?.prefer_main ?? false"
          @update:model-value="(value: boolean) => settings.mutate({ prefer_main: value })"
        />

        <div
          v-if="current?.background_supported && vault.encrypted"
          class="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border px-3 py-2"
        >
          <div class="grid min-w-0 flex-1 gap-0.5">
            <Label for="sync-background-lock">{{ t('sync.backgroundLock.label') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('sync.backgroundLock.hint') }}</p>
          </div>
          <Select
            id="sync-background-lock"
            :model-value="String(current.background_lock_min)"
            @update:model-value="(value) => settings.mutate({ background_lock_min: Number(value) })"
          >
            <SelectItem v-for="minutes in LOCK_OPTIONS" :key="minutes" :value="String(minutes)">
              {{ lockLabel(minutes) }}
            </SelectItem>
          </Select>
        </div>

        <SyncNetwork v-if="current?.enabled" />

        <p v-if="keyHint" class="text-xs text-muted-foreground" :class="current?.key_storage === 'memory' && 'text-warning'">
          {{ keyHint }}
        </p>

        <div v-if="current?.has_backup" class="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border px-3 py-2">
          <div class="grid min-w-0 flex-1 gap-0.5">
            <span class="text-sm">{{ t('sync.backup.restore') }}</span>
            <span class="text-xs text-muted-foreground">{{ t('sync.backup.hint') }}</span>
          </div>
          <ConfirmButton
            :label="t('sync.backup.restore')"
            :confirm-label="t('sync.backup.restoreConfirm')"
            :disabled="restore.isPending.value"
            @confirm="restore.mutate()"
          >
            <History class="size-3.5" />
          </ConfirmButton>
        </div>
      </div>
    </SectionState>
  </SettingsSection>
  <SyncStatusLine />
  </div>
</template>
