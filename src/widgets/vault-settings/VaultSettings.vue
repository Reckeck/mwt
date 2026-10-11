<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { ChangePasswordDialog } from '@/features/vault-change-password'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { parseError } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

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
    toast.success(t('settings.saved'))
  } catch (err) {
    toast.error(t('settings.saveFailed'), { description: parseError(err).message })
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

function updateMinimizeToTray(event: Event) {
  void persist({ minimize_to_tray: (event.target as HTMLInputElement).checked })
}
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('settings.vaultTitle') }}</p>
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
          <option v-for="opt in fxIntervalOptions" :key="opt.value" :value="String(opt.value)">{{ opt.label }}</option>
        </Select>
      </div>

      <div class="flex items-center gap-3">
        <input
          id="minimize-to-tray"
          type="checkbox"
          :checked="settingsStore.vault?.minimize_to_tray ?? true"
          class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
          @change="updateMinimizeToTray"
        >
        <Label for="minimize-to-tray">{{ t('settings.minimizeToTray') }}</Label>
      </div>

      <p class="text-xs text-muted-foreground">{{ t('settings.autoSaveHint') }}</p>

      <Button variant="outline" size="sm" class="w-fit" @click="passwordOpen = true">
        {{ t('vault.password.open') }}
      </Button>
    </div>
    <ChangePasswordDialog v-model:open="passwordOpen" />
  </Card>
</template>
