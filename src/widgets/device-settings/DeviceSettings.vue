<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery } from '@tanstack/vue-query'

import { SettingsSection } from '@/shared/ui/settings-section'
import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { usePlatform } from '@/shared/lib/platform'
import { useSettingsStore } from '@/shared/store/settings'
import { SwitchRow } from '@/shared/ui/switch-row'

/**
 * What belongs to this device, not to the vault: on a desktop the tray and
 * starting with the system. How often rates refresh lives under «Валюты».
 */
const { t } = useI18n()
const settingsStore = useSettingsStore()
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
