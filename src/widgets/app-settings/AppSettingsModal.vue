<script setup lang="ts">
import { onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'

import { CurrencyPicker } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { setLocalePreference } from '@/shared/lib/locale'
import { useSettingsStore } from '@/shared/store/settings'
import { notify } from '@/shared/lib/notify'

import FavoriteCurrenciesField from './ui/FavoriteCurrenciesField.vue'

const { t } = useI18n()
const settingsStore = useSettingsStore()
const queryClient = useQueryClient()

const themes: Theme[] = ['light', 'dark', 'auto']

const localeOptions: { code: Locale; label: string }[] = [
  { code: 'auto', label: 'settings.locale.auto' },
  { code: 'ru', label: 'settings.locale.ru' },
  { code: 'en', label: 'settings.locale.en' },
]

onMounted(async () => {
  // Normally already resolved by App.vue on boot, but the dialog can be opened
  // before that read lands (or after a failed one) — render the real values.
  if (!settingsStore.appLoaded) {
    await settingsStore.loadApp().catch(() => {
      /* the store falls back to its defaults, and every write reports itself */
    })
  }
  // Warm the catalog so the dropdown is populated before the first keystroke.
  // `loadCurrencies` is a no-op once the full list is in, so the pickers' own
  // load on open costs nothing when this already succeeded.
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
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
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
    notify.saved()
  } catch (err) {
    notify.error(err, 'settings.localeSaveFailed')
  }
}

/** Every figure in the base currency changes with it; nothing cached may survive. */
async function onBaseCurrencyChange(next: string) {
  if (!next || next === settingsStore.baseCurrency) return
  await persist({ base_currency: next })
  await queryClient.invalidateQueries()
}

</script>

<template>
  <div class="grid gap-4">
    <!-- Theme -->
    <div class="grid gap-1.5">
      <Label>{{ t('settings.theme.title') }}</Label>
      <div class="grid grid-cols-3 gap-2">
        <Button
          variant="outline"
          v-for="option in themes"
          :key="option"
          class="font-normal rounded-md border px-3 py-2 text-sm transition-colors hover:bg-accent"
          :class="settingsStore.theme === option && 'border-primary bg-accent font-medium'"
          @click="setTheme(option)"
        >
          {{ t(`settings.theme.${option}`) }}
        </Button>
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
        <SelectItem v-for="option in localeOptions" :key="option.code" :value="option.code">
          {{ t(option.label) }}
        </SelectItem>
      </Select>
    </div>

    <!-- Base currency -->
    <CurrencyPicker
      :label="t('settings.baseCurrency')"
      :model-value="settingsStore.baseCurrency"
      @update:model-value="onBaseCurrencyChange"
    />

    <FavoriteCurrenciesField />
  </div>
</template>