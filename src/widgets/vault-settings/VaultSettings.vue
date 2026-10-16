<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { ChangePasswordDialog } from '@/features/vault-change-password'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { useSettingsStore } from '@/shared/store/settings'
import { notify } from '@/shared/lib/notify'
import { Switch } from '@/shared/ui/switch'

const { t } = useI18n()
const settingsStore = useSettingsStore()
const passwordOpen = ref(false)

/** "300" → "5 min" in the UI language; the options used to be hard-coded in Russian. */
function duration(seconds: number): string {
  if (seconds % 3600 === 0) return t('duration.hours', { n: seconds / 3600 })
  if (seconds % 60 === 0) return t('duration.minutes', { n: seconds / 60 })
  return t('duration.seconds', { n: seconds })
}

const fxIntervalOptions = computed(() =>
  [30, 60, 300, 900, 1800, 3600].map((value) => ({ value, label: duration(value) })),
)

/**
 * Persist one setting and keep the store in step.
 *
 * Settings are written the moment they change, so nothing can be lost by
 * closing the app without hitting Save — which is exactly what used to happen:
 * only the language switch persisted, every other control only mutated local
 * state and silently reverted on the next launch.
 *
 * A failed write is surfaced as a toast and the store is re-read from the
 * backend, so what the screen shows is always what is on disk.
 */
async function persist(patch: Partial<Settings>) {
  try {
    await settingsStore.updateVault(patch)
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
    try {
      await settingsStore.loadVault()
    } catch {
      /* the error above is the one worth showing */
    }
  }
}

function updateFxInterval(value: string) {
  void persist({ fx_refresh_interval_sec: Number(value) })
}

</script>

<template>
  <Card>
    <template #header>
      <h3 class="heading-card">{{ t('settings.vaultTitle') }}</h3>
    </template>

    <div class="grid gap-4">
      <p class="text-xs text-muted-foreground">{{ t('settings.vaultScopeHint') }}</p>
      <div class="grid gap-1.5">
        <Label for="fx-interval">{{ t('settings.fxRefreshInterval') }}</Label>
        <Select
          id="fx-interval"
          :value="String(settingsStore.vault?.fx_refresh_interval_sec ?? 300)"
          @update:model-value="updateFxInterval"
        >
          <SelectItem v-for="opt in fxIntervalOptions" :key="opt.value" :value="String(opt.value)">{{ opt.label }}</SelectItem>
        </Select>
      </div>

      <div class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2">
        <Label for="minimize-to-tray">{{ t('settings.minimizeToTray') }}</Label>
        <Switch
          id="minimize-to-tray"
          :model-value="settingsStore.vault?.minimize_to_tray ?? true"
          @update:model-value="(value: boolean) => void persist({ minimize_to_tray: value })"
        />
      </div>

      <p class="text-xs text-muted-foreground">{{ t('settings.autoSaveHint') }}</p>

      <Button variant="outline" size="sm" class="w-fit" @click="passwordOpen = true">
        {{ t('vault.password.open') }}
      </Button>
    </div>
    <ChangePasswordDialog v-model:open="passwordOpen" />
  </Card>
</template>
