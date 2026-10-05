<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Download, Eye } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import Badge from '@/components/ui/badge.vue'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Dialog from '@/components/ui/dialog.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Select from '@/components/ui/select.vue'
import VaultJsonEditor from '@/components/settings/VaultJsonEditor.vue'
import { call, parseError, type AppError } from '@/lib/ipc'
import { setLocalePreference } from '@/locale'
import { useSettingsStore } from '@/stores/settings'

const { t } = useI18n()
const settingsStore = useSettingsStore()

const isDev = import.meta.env.DEV

const baseCurrency = ref<string>(settingsStore.settings?.base_currency ?? 'usd')
const favoriteCurrenciesInput = ref<string>('')

/* --- vault JSON (DEV): view + export --- */
const jsonDialogOpen = ref(false)

const currencies = [
  { code: 'usd', label: 'USD' },
  { code: 'eur', label: 'EUR' },
  { code: 'rub', label: 'RUB' },
  { code: 'cny', label: 'CNY' },
]

const localeOptions: { code: Locale; label: string }[] = [
  { code: 'auto', label: 'settings.locale.auto' },
  { code: 'ru', label: 'settings.locale.ru' },
  { code: 'en', label: 'settings.locale.en' },
]

const fxIntervalOptions = [
  { value: 30, label: '30 сек' },
  { value: 60, label: '1 мин' },
  { value: 300, label: '5 мин' },
  { value: 900, label: '15 мин' },
  { value: 1800, label: '30 мин' },
  { value: 3600, label: '1 час' },
]

const bybitIntervalOptions = [
  { value: 0, label: t('settings.bybit.importOff') },
  { value: 300, label: '5 мин' },
  { value: 900, label: '15 мин' },
  { value: 3600, label: '1 час' },
  { value: 21600, label: '6 час' },
]

const currentLocale = () => settingsStore.settings?.locale ?? 'auto'

// Dashboard widget definitions
const widgetDefs = [
  { id: 'total', label: 'dashboard.widget.total', defaultOrder: 0 },
  { id: 'assets', label: 'dashboard.widget.assets', defaultOrder: 1 },
  { id: 'fx', label: 'dashboard.widget.fx', defaultOrder: 2 },
  { id: 'byChain', label: 'dashboard.widget.byChain', defaultOrder: 3 },
  { id: 'byStore', label: 'dashboard.widget.byStore', defaultOrder: 4 },
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
    settingsStore.settings = await call<Settings>('update_settings', { input: toInput(patch) })
    toast.success(t('settings.saved'))
  } catch (err) {
    toast.error(t('settings.saveFailed'), { description: parseError(err).message })
    try {
      await settingsStore.load()
    } catch {
      /* the error above is the one worth showing */
    }
  }
}

/** Only the fields present in `patch` are sent; the backend ignores the rest. */
function toInput(patch: Partial<Settings>): UpdateSettingsInput {
  return {
    base_currency: patch.base_currency ?? null,
    theme: patch.theme ?? null,
    locale: patch.locale ?? null,
    display_decimals: patch.display_decimals ?? null,
    favorite_currencies: patch.favorite_currencies ?? null,
    dashboard_widgets: patch.dashboard_widgets ?? null,
    fx_refresh_interval_sec: patch.fx_refresh_interval_sec ?? null,
    minimize_to_tray: patch.minimize_to_tray ?? null,
    bybit_import_interval_sec: patch.bybit_import_interval_sec ?? null,
  }
}

function getWidgets() {
  const current = settingsStore.settings
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
  const current = settingsStore.settings
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
  const current = settingsStore.settings
  if (!current) return
  const widgets = [...(current.dashboard_widgets ?? [])]
  const idx = widgets.findIndex((widget) => widget.id === id)
  if (idx < 0) return
  const newOrder = Math.max(0, widgets[idx].order + delta)
  widgets[idx] = { ...widgets[idx], order: newOrder }
  widgets.sort((left, right) => left.order - right.order)
  void persist({ dashboard_widgets: widgets })
}

/**
 * Language applies immediately — the UI must switch while the user is still
 * looking at the control — so it persists right away too.
 */
async function onLocaleChange(event: Event) {
  const next = (event.target as HTMLSelectElement).value as Locale
  try {
    await setLocalePreference(next)
  } catch {
    toast.error(t('settings.localeSaveFailed'))
  }
}

function addFavoriteCurrency() {
  const code = favoriteCurrenciesInput.value.trim().toUpperCase()
  if (code.length !== 3) return
  const current = settingsStore.settings
  if (!current) return
  const currencies = [...(current.favorite_currencies ?? [])]
  if (!currencies.includes(code)) {
    currencies.push(code)
    favoriteCurrenciesInput.value = ''
    void persist({ favorite_currencies: currencies })
  }
}

function removeFavoriteCurrency(curr: string) {
  const current = settingsStore.settings
  if (!current) return
  void persist({
    favorite_currencies: (current.favorite_currencies ?? []).filter((code) => code !== curr),
  })
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

function updateBaseCurrency(value: string) {
  baseCurrency.value = value
  void persist({ base_currency: value })
}

function setTheme(theme: Theme) {
  if (!settingsStore.settings || settingsStore.settings.theme === theme) return
  void persist({ theme })
}

async function devDumpVault() {
  try {
    // Native save dialog + write happen in Rust: the webview cannot download
    // files on its own, and `dev_export_vault` returns null when cancelled.
    const written = await call<string | null>('dev_export_vault')
    if (written) toast.success(t('settings.dev.savedTo'), { description: written })
  } catch (err) {
    toast.error(t('settings.dev.dumpFailed'), { description: errorText(parseError(err)) })
  }
}

/** The editor component loads the document itself on mount. */
function viewVaultJson() {
  jsonDialogOpen.value = true
}

/** Show a known error code as its localized text; fall back to the raw message. */
function errorText(err: AppError): string {
  const known = t(`errors.${err.code}`)
  return known === `errors.${err.code}` ? err.message : known
}
</script>

<template>
  <div class="mx-auto grid max-w-3xl gap-4">
    <Card>
      <template #header>
        <p class="font-medium">{{ t('settings.title') }}</p>
      </template>

      <div class="grid gap-4">
        <div class="grid gap-1.5">
          <Label>{{ t('settings.theme.title') }}</Label>
          <div v-if="settingsStore.settings" class="grid grid-cols-3 gap-2">
            <button
              v-for="th in (['light', 'dark', 'auto'] as const)"
              :key="th"
              type="button"
              class="rounded-md border px-3 py-2 text-sm transition-colors hover:bg-accent"
              :class="settingsStore.settings.theme === th && 'border-primary bg-accent'"
              @click="setTheme(th)"
            >
              {{ t(`settings.theme.${th}`) }}
            </button>
          </div>
        </div>

        <div class="grid gap-1.5">
          <Label for="bcur">{{ t('settings.baseCurrency') }}</Label>
          <select
            id="bcur"
            :value="settingsStore.settings?.base_currency ?? baseCurrency"
            class="h-9 rounded-md border border-input bg-transparent px-3 text-sm"
            @change="updateBaseCurrency(($event.target as HTMLSelectElement).value)"
          >
            <option v-for="currency in currencies" :key="currency.code" :value="currency.code">
              {{ currency.label }}
            </option>
          </select>
        </div>

        <div class="grid gap-1.5">
          <Label for="loc">{{ t('settings.language') }}</Label>
          <select
            id="loc"
            :value="currentLocale()"
            class="h-9 rounded-md border border-input bg-transparent px-3 text-sm"
            @change="onLocaleChange"
          >
            <option v-for="locale in localeOptions" :key="locale.code" :value="locale.code">
              {{ t(locale.label) }}
            </option>
          </select>
        </div>

        <div class="grid gap-1.5">
          <Label>{{ t('settings.favoriteCurrencies') }}</Label>
          <div class="flex flex-wrap gap-2 items-center">
            <div class="flex items-center gap-1.5 flex-wrap">
              <span
                v-for="curr in settingsStore.settings?.favorite_currencies ?? []"
                :key="curr"
                class="inline-flex items-center gap-1 px-2 py-1 text-sm bg-accent rounded-md text-accent-foreground"
              >
                {{ curr }}
                <button
                  type="button"
                  class="text-xs hover:text-destructive transition-colors"
                  @click="removeFavoriteCurrency(curr)"
                >
                  ×
                </button>
              </span>
            </div>
            <div class="flex items-center gap-1.5">
              <Input
                v-model="favoriteCurrenciesInput"
                placeholder="USD"
                class="w-24 h-9 text-sm"
                @keyup.enter="addFavoriteCurrency"
              />
              <Button size="sm" @click="addFavoriteCurrency">{{ t('settings.favoriteCurrencyAdd') }}</Button>
            </div>
            <p class="text-xs text-muted-foreground">{{ t('settings.favoriteCurrenciesHint') }}</p>
          </div>
        </div>

        <div class="grid gap-1.5">
          <Label for="fx-interval">{{ t('settings.fxRefreshInterval') }}</Label>
          <Select
            id="fx-interval"
            :value="String(settingsStore.settings?.fx_refresh_interval_sec ?? 300)"
            @update:model-value="updateFxInterval"
          >
            <option v-for="opt in fxIntervalOptions" :key="opt.value" :value="String(opt.value)">{{ opt.label }}</option>
          </Select>
        </div>

        <div class="grid gap-1.5">
          <Label for="bybit-import-interval">{{ t('settings.bybit.importInterval') }}</Label>
          <Select
            id="bybit-import-interval"
            :value="String(settingsStore.settings?.bybit_import_interval_sec ?? 900)"
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
            :checked="settingsStore.settings?.minimize_to_tray ?? true"
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

    <Card v-if="isDev">
      <template #header>
        <div class="flex items-center justify-between">
          <p class="font-medium">{{ t('settings.dev.openVault') }}</p>
          <Badge variant="secondary" class="text-xs">DEV</Badge>
        </div>
      </template>
      <div class="grid gap-2">
        <div class="flex flex-wrap items-center gap-2">
          <Button variant="outline" @click="viewVaultJson">
            <Eye class="size-4" />
            {{ t('settings.dev.viewVault') }}
          </Button>
          <Button variant="outline" @click="devDumpVault">
            <Download class="size-4" />
            {{ t('settings.dev.dumpVault') }}
          </Button>
          <p class="text-xs text-muted-foreground">{{ t('settings.dev.dumpHint') }}</p>
        </div>
      </div>
    </Card>

    <Dialog
      v-model:open="jsonDialogOpen"
      :title="t('settings.dev.vaultJsonTitle')"
      :description="t('settings.dev.vaultJsonHint')"
      size="xl"
    >
      <!-- Mounted per open so every visit starts from the live document. -->
      <VaultJsonEditor v-if="jsonDialogOpen" />
      <template #footer>
        <Button @click="jsonDialogOpen = false">{{ t('actions.close') }}</Button>
      </template>
    </Dialog>
  </div>
</template>
