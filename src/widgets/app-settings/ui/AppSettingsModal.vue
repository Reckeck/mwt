<script setup lang="ts">
import { onMounted } from 'vue'
import { useI18n } from 'vue-i18n'

import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { setLocalePreference } from '@/shared/lib/locale'
import { useSettingsStore } from '@/shared/store/settings'
import { notify } from '@/shared/lib/notify'

const { t } = useI18n()
const settingsStore = useSettingsStore()

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

</script>

<template>
  <div class="grid gap-4">
    <!-- Theme -->
    <div class="grid gap-1.5">
      <Label>{{ t('settings.theme.title') }}</Label>
      <ToggleGroup
        type="single"
        variant="segmented"
        :spacing="1"
        class="max-w-full"
        :model-value="settingsStore.theme"
        :aria-label="t('settings.theme.title')"
        @update:model-value="(value) => value && setTheme(value as Theme)"
      >
        <ToggleGroupItem v-for="option in themes" :key="option" :value="option" class="h-10 px-3 md:px-5">
          {{ t(`settings.theme.${option}`) }}
        </ToggleGroupItem>
      </ToggleGroup>
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
  </div>
</template>