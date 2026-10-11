<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { CurrencyPicker } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { parseError } from '@/shared/api/ipc'
import { setLocalePreference } from '@/shared/lib/locale'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settingsStore = useSettingsStore()
const queryClient = useQueryClient()

/** Currency staged in the picker, waiting for the Add button. */
const pickedFavorite = ref('')

const themes: Theme[] = ['light', 'dark', 'auto']

const localeOptions: { code: Locale; label: string }[] = [
  { code: 'auto', label: 'settings.locale.auto' },
  { code: 'ru', label: 'settings.locale.ru' },
  { code: 'en', label: 'settings.locale.en' },
]

/** Favorites never include the base currency — it would show the same number twice. */
const favorites = computed(() =>
  settingsStore.favoriteCurrencies.filter((code) => code.toLowerCase() !== settingsStore.baseCurrency),
)

onMounted(async () => {
  // Normally already resolved by App.vue on boot, but the dialog can be opened
  // before that read lands (or after a failed one) — render the real values.
  if (!settingsStore.appLoaded) {
    await settingsStore.loadApp().catch(() => {
      /* the store falls back to its defaults, and every write reports itself */
    })
  }
  // Warm the catalog so the dropdown is populated before the first keystroke.
  // `loadCurrencies` is a no-op once it has run, so the pickers' own
  // keystroke-triggered load costs nothing when this already succeeded.
  await settingsStore.loadCurrencies().catch(() => {
    /* the picker shows a notice and falls back to the built-in list */
  })
})

/**
 * Persist one setting and keep the store in step.
 *
 * Every control writes the moment it changes — there is no Save button, so a
 * failed write must be surfaced and the store re-read, otherwise the screen
 * would show a value that is not on disk.
 */
async function persist(patch: Partial<UpdateAppSettingsInput>) {
  try {
    await settingsStore.updateApp(patch)
    toast.success(t('settings.saved'))
  } catch (err) {
    toast.error(t('settings.saveFailed'), { description: parseError(err).message })
    // These are app settings: re-read them, not the vault's.
    await settingsStore.loadApp().catch(() => {
      /* the error above is the one worth showing */
    })
  }
}

function setTheme(theme: Theme) {
  if (settingsStore.theme === theme) return
  void persist({ theme })
}

/** The language must switch while the user is still looking at the control. */
async function onLocaleChange(next: string) {
  try {
    await setLocalePreference(next as Locale)
    toast.success(t('settings.saved'))
  } catch (err) {
    toast.error(t('settings.localeSaveFailed'), { description: parseError(err).message })
  }
}

/** Every figure in the base currency changes with it; nothing cached may survive. */
async function onBaseCurrencyChange(next: string) {
  if (!next || next === settingsStore.baseCurrency) return
  await persist({ base_currency: next })
  await queryClient.invalidateQueries()
}

function addFavorite() {
  const code = pickedFavorite.value.trim().toLowerCase()
  if (!code) return
  if (code === settingsStore.baseCurrency || favorites.value.some((entry) => entry.toLowerCase() === code)) {
    pickedFavorite.value = ''
    return
  }
  pickedFavorite.value = ''
  void persist({ favorite_currencies: [...settingsStore.favoriteCurrencies, code.toUpperCase()] })
}

function removeFavorite(code: string) {
  void persist({
    favorite_currencies: settingsStore.favoriteCurrencies.filter((entry) => entry !== code),
  })
}
</script>

<template>
  <div class="grid gap-4">
    <!-- Theme -->
    <div class="grid gap-1.5">
      <Label>{{ t('settings.theme.title') }}</Label>
      <div class="grid grid-cols-3 gap-2">
        <button
          v-for="option in themes"
          :key="option"
          type="button"
          class="rounded-md border px-3 py-2 text-sm transition-colors hover:bg-accent"
          :class="settingsStore.theme === option && 'border-primary bg-accent font-medium'"
          @click="setTheme(option)"
        >
          {{ t(`settings.theme.${option}`) }}
        </button>
      </div>
    </div>

    <!-- Language -->
    <div class="grid gap-1.5">
      <Label for="app-locale">{{ t('settings.language') }}</Label>
      <Select
        id="app-locale"
        :model-value="settingsStore.locale"
        @update:model-value="onLocaleChange($event)"
      >
        <option v-for="option in localeOptions" :key="option.code" :value="option.code">
          {{ t(option.label) }}
        </option>
      </Select>
    </div>

    <!-- Base currency -->
    <CurrencyPicker
      :label="t('settings.baseCurrency')"
      :model-value="settingsStore.baseCurrency"
      @update:model-value="onBaseCurrencyChange"
    />

    <!-- Favorite currencies -->
    <div class="grid gap-1.5">
      <Label>{{ t('settings.favoriteCurrencies') }}</Label>
      <div v-if="favorites.length" class="flex flex-wrap gap-2">
        <span
          v-for="code in favorites"
          :key="code"
          class="inline-flex items-center gap-1 rounded-md bg-accent px-2 py-1 text-sm text-accent-foreground"
        >
          {{ code }}
          <button
            type="button"
            class="text-xs transition-colors hover:text-destructive"
            :aria-label="t('settings.favoriteCurrencyRemove', { code })"
            @click="removeFavorite(code)"
          >
            ×
          </button>
        </span>
      </div>
      <div class="flex items-end gap-2">
        <CurrencyPicker
          v-model="pickedFavorite"
          class="flex-1"
          :placeholder="t('settings.favoriteCurrencyPlaceholder')"
        />
        <Button
          size="sm"
          class="h-9"
          :disabled="!pickedFavorite"
          @click="addFavorite"
        >
          {{ t('settings.favoriteCurrencyAdd') }}
        </Button>
      </div>
      <p class="text-xs text-muted-foreground">{{ t('settings.favoriteCurrenciesHint') }}</p>
    </div>
  </div>
</template>