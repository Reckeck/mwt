<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { EDITOR_HOTKEYS } from '../model/hotkeys'

/** The editor's keyboard shortcuts. */
const { t } = useI18n()
</script>

<template>
  <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
    <template v-for="hotkey in EDITOR_HOTKEYS" :key="hotkey.action">
      <dt><kbd class="rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-xs">{{ hotkey.keys }}</kbd></dt>
      <dd class="text-muted-foreground">{{ t(`workflow.hotkey.${hotkey.action}`) }}</dd>
    </template>
  </dl>
</template>
