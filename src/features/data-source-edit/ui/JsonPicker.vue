<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import JsonNode from './JsonNode.vue'

/** A JSON answer as a tree: a click on a value gives its path; an array is a series. */
const props = defineProps<{ json: string; selected: string }>()
const emit = defineEmits<{ pick: [path: string, isArray: boolean] }>()
const { t } = useI18n()

const parsed = computed<unknown>(() => {
  try {
    return JSON.parse(props.json)
  } catch {
    return null
  }
})
</script>

<template>
  <div class="grid gap-1.5">
    <p class="text-xs text-muted-foreground">{{ t('sources.picker.jsonHint') }}</p>
    <div class="h-80 overflow-auto rounded-md border p-1 [scrollbar-gutter:stable] md:h-96" data-tour="source-picker">
      <JsonNode
        name="$"
        :value="parsed"
        path=""
        :selected="props.selected"
        :depth="0"
        @pick="(path, array) => emit('pick', path, array)"
      />
    </div>
  </div>
</template>
