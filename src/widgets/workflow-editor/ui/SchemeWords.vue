<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import type { WordPart } from '../model/scheme-words'

/** «Словами: когда … → спросить … → перевести …» over the canvas (Auto-2 · 13). */
defineProps<{ parts: WordPart[] }>()
const { t } = useI18n()
</script>

<template>
  <p class="text-sm text-muted-foreground" data-tour="workflow-words">
    {{ t('workflow.words.label') }}
    <template v-if="parts.length">
      <template v-for="(part, index) in parts" :key="index">
        <strong v-if="part.strong" class="font-semibold text-foreground">{{ part.text }}</strong>
        <template v-else>{{ part.text }}</template>
      </template>
    </template>
    <template v-else>{{ t('workflow.words.empty') }}</template>
  </p>
</template>
