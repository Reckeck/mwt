<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery } from '@tanstack/vue-query'

import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { SettingsSection } from '@/shared/ui/settings-section'
import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { usePlatform } from '@/shared/lib/platform'
import { useSettingsStore } from '@/shared/store/settings'
import { SwitchRow } from '@/shared/ui/switch-row'

/**
 * What belongs to this device, not to the vault: how often prices refresh,
 * and on a desktop the tray and starting with the system.
 */
const { t } = useI18n()
const settingsStore = useSettingsStore()
const { isDesktop } = usePlatform()

/** "300" → "5 min" in the UI language. */
function duration(seconds: number): string {
  if (seconds % 3600 === 0) return t('duration.hours', { n: seconds / 3600 })
  if (seconds % 60 === 0) return t('duration.minutes', { n: seconds / 60 })
  return t('duration.seconds', { n: seconds })
}
const fxIntervalOptions = computed(() => [30, 60, 300, 900, 1800, 3600].map((value) => ({ value, label: duration(value) })))

async function persistDevice(patch: Partial<UpdateAppSettingsInput>) {
  try {
    await settingsStore.updateApp(patch)
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
    await settingsStore.loadApp().catch(() => undefined)
  }
}

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
    <div class="grid gap-4">
      <div class="grid gap-1.5">
        <Label for="fx-interval">{{ t('settings.fxRefreshInterval') }}</Label>
        <Select
          id="fx-interval"
          :model-value="String(settingsStore.app?.fx_refresh_interval_sec ?? settingsStore.vault?.fx_refresh_interval_sec ?? 300)"
          @update:model-value="(value) => void persistDevice({ fx_refresh_interval_sec: Number(value) })"
        >
          <SelectItem v-for="opt in fxIntervalOptions" :key="opt.value" :value="String(opt.value)">{{ opt.label }}</SelectItem>
        </Select>
      </div>

      <template v-if="isDesktop">
        <SwitchRow
          id="minimize-to-tray"
          :label="t('settings.minimizeToTray')"
          :model-value="settingsStore.app?.minimize_to_tray ?? settingsStore.vault?.minimize_to_tray ?? true"
          @update:model-value="(value: boolean) => void persistDevice({ minimize_to_tray: value })"
        />
        <SwitchRow
          id="device-autostart"
          :label="t('notices.autostart')"
          :hint="t('notices.autostartHint')"
          :model-value="autostart.data.value ?? false"
          :disabled="setAutostart.isPending.value"
          @update:model-value="(value) => setAutostart.mutate(Boolean(value))"
        />
      </template>
    </div>
  </SettingsSection>
</template>
