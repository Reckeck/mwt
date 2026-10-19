<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import type { EditorNoteData } from '../model/flow'

/** A note on the canvas: what a part of the workflow is for. Runs ignore it. */
const props = defineProps<{ id: string; data: EditorNoteData; selected?: boolean }>()
const emit = defineEmits<{ text: [id: string, value: string] }>()

const { t } = useI18n()
// check-ui-ignore fixed-width: a note on the canvas, which pans and zooms
</script>

<template>
  <div
    class="w-56 rounded-lg border bg-muted p-2 shadow-sm"
    :class="props.selected ? 'border-primary' : 'border-border'"
  >
    <textarea
      class="nodrag nowheel block min-h-16 w-full resize-y bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
      :value="props.data.text"
      :placeholder="t('workflow.note.placeholder')"
      :aria-label="t('workflow.note.label')"
      rows="3"
      @input="emit('text', props.id, ($event.target as HTMLTextAreaElement).value)"
    />
  </div>
</template>
