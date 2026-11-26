<script setup lang="ts">
import { Textarea } from '@/shared/ui/textarea'
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
    class="w-56 rounded-lg border bg-muted p-2 shadow-sm focus-within:ring-2 focus-within:ring-ring"
    :class="props.selected ? 'border-primary' : 'border-border'"
  >
    <Textarea
      class="nodrag nowheel block min-h-16 border-0 bg-transparent px-0 py-0 text-xs"
      :model-value="props.data.text"
      :placeholder="t('workflow.note.placeholder')"
      :aria-label="t('workflow.note.label')"
      rows="3"
      @update:model-value="(text) => emit('text', props.id, String(text))"
    />
  </div>
</template>
