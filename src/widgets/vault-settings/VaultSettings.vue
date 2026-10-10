<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { parseError } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settingsStore = useSettingsStore()

/** "300" → "5 min" in the UI language; the options used to be hard-coded in Russian. */
function duration(seconds: number): string {
  if (seconds % 3600 === 0) return t('duration.hours', { n: seconds / 3600 })
  if (seconds % 60 === 0) return t('duration.minutes', { n: seconds / 60 })
  return t('duration.seconds', { n: seconds })
}

const fxIntervalOptions = computed(() =>
  [30, 60, 300, 900, 1800, 3600].map((value) => ({ value, label: duration(value) })),
)

const bybitIntervalOptions = computed(() => [
  { value: 0, label: t('settings.bybit.importOff') },
  ...[300, 900, 3600, 21600].map((value) => ({ value, label: duration(value) })),
])

// Dashboard widget definitions
const widgetDefs = [
  { id: 'total', label: 'dashboard.widget.total', defaultOrder: 0 },
  { id: 'assets', label: 'dashboard.widget.assets', defaultOrder: 1 },
  { id: 'fx', label: 'dashboard.widget.fx', defaultOrder: 2 },
  { id: 'byChain', label: 'dashboard.widget.byChain', defaultOrder: 3 },
  { id: 'byStore', label: 'dashboard.widget.byStore', defaultOrder: 4 },
  { id: 'dailyBudget', label: 'dashboard.widget.dailyBudget', defaultOrder: 5 },
  { id: 'monthSummary', label: 'dashboard.widget.monthSummary', defaultOrder: 6 },
  { id: 'upcoming', label: 'dashboard.widget.upcoming', defaultOrder: 7 },
  { id: 'goals', label: 'dashboard.widget.goals', defaultOrder: 8 },
  { id: 'budgets', label: 'dashboard.widget.budgets', defaultOrder: 9 },
  { id: 'spending7d', label: 'dashboard.widget.spending7d', defaultOrder: 10 },
  { id: 'miniCalendar', label: 'dashboard.widget.miniCalendar', defaultOrder: 11 },
  { id: 'netWorth', label: 'dashboard.widget.netWorth', defaultOrder: 12 },
  { id: 'fixedCosts', label: 'dashboard.widget.fixedCosts', defaultOrder: 13 },
] as const

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

function getWidgets() {
  const current = settingsStore.vault
  if (!current) return []
  const widgets = current.dashboard_widgets ?? []
  return widgetDefs.map((def) => {
    const existing = widgets.find((widget) => widget.id === def.id)
    return {
      id: def.id,
      label: t(def.label),
      visible: existing?.visible ?? true,
      order: existing?.order ?? def.defaultOrder,
    }
  }).sort((left, right) => left.order - right.order)
}

function toggleWidget(id: string) {
  const current = settingsStore.vault
  if (!current) return
  const widgets = [...(current.dashboard_widgets ?? [])]
  const idx = widgets.findIndex((widget) => widget.id === id)
  if (idx >= 0) {
    widgets[idx] = { ...widgets[idx], visible: !widgets[idx].visible }
  } else {
    widgets.push({ id, visible: true, order: widgetDefs.find((def) => def.id === id)?.defaultOrder ?? 0 })
  }
  void persist({ dashboard_widgets: widgets })
}

function moveWidget(id: string, delta: number) {
  const current = settingsStore.vault
  if (!current) return
  const widgets = [...(current.dashboard_widgets ?? [])]
  const idx = widgets.findIndex((widget) => widget.id === id)
  if (idx < 0) return
  const newOrder = Math.max(0, widgets[idx].order + delta)
  widgets[idx] = { ...widgets[idx], order: newOrder }
  widgets.sort((left, right) => left.order - right.order)
  void persist({ dashboard_widgets: widgets })
}

function updateFxInterval(value: string) {
  void persist({ fx_refresh_interval_sec: Number(value) })
}

function updateBybitImportInterval(value: string) {
  void persist({ bybit_import_interval_sec: Number(value) })
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

      <div class="grid gap-1.5">
        <Label for="bybit-import-interval">{{ t('settings.bybit.importInterval') }}</Label>
        <Select
          id="bybit-import-interval"
          :value="String(settingsStore.vault?.bybit_import_interval_sec ?? 900)"
          @update:model-value="updateBybitImportInterval"
        >
          <option v-for="opt in bybitIntervalOptions" :key="opt.value" :value="String(opt.value)">
            {{ opt.label }}
          </option>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t('settings.bybit.importIntervalHint') }}</p>
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

      <div class="grid gap-1.5 pt-2">
        <Label>{{ t('settings.dashboardWidgets') }}</Label>
        <p class="text-xs text-muted-foreground">{{ t('settings.dashboardWidgetsHint') }}</p>
        <div class="space-y-2">
          <div
            v-for="widget in getWidgets()"
            :key="widget.id"
            class="flex items-center gap-3 p-2 rounded-md border border-border bg-card"
          >
            <input
              :id="`widget-${widget.id}`"
              type="checkbox"
              class="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary"
              :checked="widget.visible"
              @change="toggleWidget(widget.id)"
            >
            <Label :for="`widget-${widget.id}`" class="flex-1 cursor-pointer">{{ widget.label }}</Label>
            <div class="flex items-center gap-1">
              <Button
                variant="ghost"
                size="icon"
                title="Move up"
                :disabled="widget.order === 0"
                @click="moveWidget(widget.id, -10)"
              >
                <span class="size-4">↑</span>
              </Button>
              <Button
                variant="ghost"
                size="icon"
                title="Move down"
                @click="moveWidget(widget.id, 10)"
              >
                <span class="size-4">↓</span>
              </Button>
            </div>
          </div>
        </div>
      </div>

      <p class="text-xs text-muted-foreground">{{ t('settings.autoSaveHint') }}</p>
    </div>
  </Card>
</template>
