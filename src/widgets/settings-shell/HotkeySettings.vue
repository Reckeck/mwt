<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { LISTED_HOTKEYS, hotkeyLabel } from '@/shared/lib/hotkeys'
import { usePlatform } from '@/shared/lib/platform'
import { Kbd } from '@/shared/ui/kbd'
import { SettingsSection } from '@/shared/ui/settings-section'

/** Keys inside the app, read from the registry; the sidebar fold has no phone. */
const { t } = useI18n()
const { isMobile } = usePlatform()
const rows = computed(() => LISTED_HOTKEYS.filter((hotkey) => !hotkey.desktopOnly || !isMobile.value))
</script>

<template>
  <SettingsSection :title="t('settingsShell.section.hotkeys')">
    <ul class="grid gap-1">
      <li v-for="hotkey in rows" :key="hotkey.id" 
        :class="['min-h-10 items-center justify-between gap-3 text-sm', hotkey.desktopOnly ? 'hidden md:flex' : 'flex']"
      >
        <span class="text-muted-foreground">{{ t(hotkey.key) }}</span>
        <Kbd class="font-mono tracking-normal">{{ hotkeyLabel(hotkey.id) }}</Kbd>
      </li>
    </ul>
  </SettingsSection>
</template>
