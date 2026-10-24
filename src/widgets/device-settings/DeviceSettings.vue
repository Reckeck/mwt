<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { SYNC_STATUS_KEY } from '@/features/vault-sync'
import { SettingsSection } from '@/shared/ui/settings-section'
import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { usePlatform } from '@/shared/lib/platform'
import { useSettingsStore } from '@/shared/store/settings'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'

/**
 * What belongs to this device, not to the vault: its name, how often prices
 * refresh and, on a desktop, the tray and starting with the system.
 */
const { t } = useI18n()
const settingsStore = useSettingsStore()
const queryClient = useQueryClient()
const { isDesktop } = usePlatform()

async function persistDevice(patch: Partial<UpdateAppSettingsInput>) {
  try {
    await settingsStore.updateApp(patch)
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
    await settingsStore.loadApp().catch(() => undefined)
  }
}

const name = ref('')
watch(
  () => settingsStore.app?.device_name,
  (value) => {
    name.value = value ?? ''
  },
  { immediate: true },
)
async function saveName() {
  if (name.value.trim() === (settingsStore.app?.device_name ?? '')) return
  await persistDevice({ device_name: name.value.trim() })
  void queryClient.invalidateQueries({ queryKey: SYNC_STATUS_KEY })
}

const INTERVALS = [30, 60, 300, 900, 1800, 3600]
const interval = computed(() => String(settingsStore.app?.fx_refresh_interval_sec ?? settingsStore.vault?.fx_refresh_interval_sec ?? 300))

const autostart = useQuery({
  queryKey: ['autostart'],
  queryFn: () => call<boolean>('get_autostart'),
  enabled: isDesktop,
})
const setAutostart = useMutation({
  mutationFn: (enabled: boolean) => call<boolean>('set_autostart', { enabled }),
  onSuccess: () => void autostart.refetch(),
  meta: {
    error: 'settings.saveFailed',
    success: (data) => (data ? t('notices.autostartOn') : t('notices.autostartOff')),
  },
})
</script>

<template>
  <SettingsSection :title="t('settingsShell.section.deviceApp')" :description="t('settings.deviceScopeHint')">
    <div class="grid gap-4 @md:grid-cols-2">
    <FormField for="device-name" :label="t('settings.deviceName')" :hint="t('settings.deviceNameHint')">
      <Input id="device-name" v-model="name" maxlength="40" @blur="saveName" @keydown.enter.prevent="saveName" />
    </FormField>
    <FormField for="device-refresh" :label="t('settings.refreshPrices')">
      <Select id="device-refresh" :model-value="interval" @update:model-value="(value) => persistDevice({ fx_refresh_interval_sec: Number(value) })">
        <SelectItem v-for="seconds in INTERVALS" :key="seconds" :value="String(seconds)">{{ t(`settings.refreshEvery.${seconds}`) }}</SelectItem>
      </Select>
    </FormField>
    </div>
    <template v-if="isDesktop">
      <SwitchRow
        plain
        id="minimize-to-tray"
        :label="t('settings.minimizeToTray')"
        :model-value="settingsStore.app?.minimize_to_tray ?? settingsStore.vault?.minimize_to_tray ?? true"
        @update:model-value="(value: boolean) => void persistDevice({ minimize_to_tray: value })"
      />
      <SwitchRow
        plain
        id="device-autostart"
        :label="t('notices.autostart')"
        :hint="t('notices.autostartHint')"
        :model-value="autostart.data.value ?? false"
        :disabled="setAutostart.isPending.value"
        @update:model-value="(value) => setAutostart.mutate(Boolean(value))"
      />
    </template>
  </SettingsSection>
</template>
