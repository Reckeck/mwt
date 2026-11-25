<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { ThemePicker } from '@/features/theme-picker'
import { themeOfMode } from '@/shared/lib/theme'
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
      :selected-id="themeOfMode(settingsStore.themeId, singleMode).id"
      @select="emit('change', { theme_id: $event })"
    />
    <template v-else>
      <div class="grid gap-1.5" data-slot="light">
        <Label>{{ t('settings.theme.lightSlot') }}</Label>
        <ThemePicker
          modes="light"
          :selected-id="themeOfMode(settingsStore.themeLightId, 'light').id"
          @select="emit('change', { theme_light_id: $event })"
        />
      </div>
      <div class="grid gap-1.5" data-slot="dark">
        <Label>{{ t('settings.theme.darkSlot') }}</Label>
        <ThemePicker
          modes="dark"
          :selected-id="themeOfMode(settingsStore.themeDarkId, 'dark').id"
          @select="emit('change', { theme_dark_id: $event })"
        />
      </div>
    </template>
  </div>
</template>
