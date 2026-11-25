<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { ThemePicker } from '@/features/theme-picker'
import { getTheme } from '@/shared/lib/theme/registry'
import type { ThemeMode } from '@/shared/lib/theme/types'
import { Label } from '@/shared/ui/label'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { useSettingsStore } from '@/shared/store/settings'

/**
 * Appearance block: the Light / Dark / Auto mode and the theme pickers.
 * Light and Dark show one picker (`themeId`); Auto shows the pair
 * (`themeLightId`, `themeDarkId`). Every id stays stored when the mode changes.
 */
const emit = defineEmits<{ change: [patch: Partial<UpdateAppSettingsInput>] }>()

const { t } = useI18n()
const settingsStore = useSettingsStore()

const modes: Theme[] = ['light', 'dark', 'auto']

/** A stored id counts only when it exists with the wanted mode; otherwise Default of the mode (TP8). */
function selectedOf(id: string | null, mode: ThemeMode): string {
  const found = id ? getTheme(id) : undefined
  return found && found.mode === mode ? found.id : `default-${mode}`
}

const singleMode = computed<ThemeMode | null>(() => (settingsStore.theme === 'auto' ? null : settingsStore.theme))

function setMode(mode: Theme) {
  if (settingsStore.theme !== mode) emit('change', { theme: mode })
}
</script>

<template>
  <div class="grid gap-4">
    <div class="grid gap-1.5">
      <Label>{{ t('settings.theme.title') }}</Label>
      <ToggleGroup
        type="single"
        variant="segmented"
        :spacing="1"
        class="max-w-full"
        :model-value="settingsStore.theme"
        :aria-label="t('settings.theme.title')"
        @update:model-value="(value) => value && setMode(value as Theme)"
      >
        <ToggleGroupItem v-for="option in modes" :key="option" :value="option" class="h-10 px-3 md:px-5">
          {{ t(`settings.theme.${option}`) }}
        </ToggleGroupItem>
      </ToggleGroup>
    </div>

    <ThemePicker
      v-if="singleMode"
      :modes="singleMode"
      :selected-id="selectedOf(settingsStore.themeId, singleMode)"
      @select="emit('change', { theme_id: $event })"
    />
    <template v-else>
      <div class="grid gap-1.5" data-slot="light">
        <Label>{{ t('settings.theme.lightSlot') }}</Label>
        <ThemePicker
          modes="light"
          :selected-id="selectedOf(settingsStore.themeLightId, 'light')"
          @select="emit('change', { theme_light_id: $event })"
        />
      </div>
      <div class="grid gap-1.5" data-slot="dark">
        <Label>{{ t('settings.theme.darkSlot') }}</Label>
        <ThemePicker
          modes="dark"
          :selected-id="selectedOf(settingsStore.themeDarkId, 'dark')"
          @select="emit('change', { theme_dark_id: $event })"
        />
      </div>
    </template>
  </div>
</template>
