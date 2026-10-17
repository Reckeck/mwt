<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { History } from 'lucide-vue-next'

import { useSyncActions, useSyncStatus } from '@/features/vault-sync'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { SectionState } from '@/shared/ui/section-state'
import { SettingsSection } from '@/shared/ui/settings-section'
import { Switch } from '@/shared/ui/switch'

/** This device's sync switches: on/off, its name, «спрашивать», «главное». */
const { t } = useI18n()
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

const keyHint = computed(() => {
  const storage = current.value?.key_storage
  return storage ? t(`sync.key.${storage}`) : ''
})
</script>

<template>
  <SettingsSection :title="t('sync.title')" :description="t('sync.description')">
    <SectionState :loading="status.isPending.value" :error="status.error.value" skeleton="list" compact @retry="status.refetch()">
      <div class="grid gap-4">
        <div class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2">
          <div class="grid gap-0.5">
            <Label for="sync-enabled">{{ t('sync.enabled') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('sync.enabledHint') }}</p>
          </div>
          <Switch
            id="sync-enabled"
            :model-value="current?.enabled ?? false"
            :disabled="settings.isPending.value"
            @update:model-value="(value: boolean) => settings.mutate({ enabled: value })"
          />
        </div>

        <FormField :label="t('sync.deviceName')" for="sync-device-name" :hint="t('sync.deviceNameHint')">
          <Input id="sync-device-name" v-model="name" maxlength="40" :placeholder="current?.device_name" @blur="saveName" @keydown.enter.prevent="saveName" />
        </FormField>

        <div class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2">
          <div class="grid gap-0.5">
            <Label for="sync-confirm">{{ t('sync.confirmFirst') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('sync.confirmFirstHint') }}</p>
          </div>
          <Switch
            id="sync-confirm"
            :model-value="current?.confirm_first ?? false"
            @update:model-value="(value: boolean) => settings.mutate({ confirm_first: value })"
          />
        </div>

        <div class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2">
          <div class="grid gap-0.5">
            <Label for="sync-main">{{ t('sync.preferMain') }}</Label>
            <p class="text-xs text-muted-foreground">{{ t('sync.preferMainHint') }}</p>
          </div>
          <Switch
            id="sync-main"
            :model-value="current?.prefer_main ?? false"
            @update:model-value="(value: boolean) => settings.mutate({ prefer_main: value })"
          />
        </div>

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
</template>
