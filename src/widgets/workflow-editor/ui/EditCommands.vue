<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ClipboardPaste, Copy, CopyPlus, Redo2, Undo2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'

import type { EditCommandSet } from '../model/edit-commands'

/**
 * Undo, redo, duplicate, copy, paste as buttons: the shortcuts alone left them
 * out of reach of a mouse-only user and of a touch screen (rule 16).
 */
const props = defineProps<{ commands: EditCommandSet }>()
const { t } = useI18n()
</script>

<template>
  <fieldset class="m-0 flex min-w-0 items-center gap-0.5 border-0 p-0">
    <legend class="sr-only">{{ t('workflow.edit.group') }}</legend>
    <Button
      variant="ghost"
      size="icon-sm"
      :disabled="!props.commands.canUndo()"
      :title="t('workflow.edit.undo')"
      :aria-label="t('workflow.edit.undo')"
      @click="props.commands.undo()"
    >
      <Undo2 />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      :disabled="!props.commands.canRedo()"
      :title="t('workflow.edit.redo')"
      :aria-label="t('workflow.edit.redo')"
      @click="props.commands.redo()"
    >
      <Redo2 />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      :title="t('workflow.edit.duplicate')"
      :aria-label="t('workflow.edit.duplicate')"
      @click="props.commands.duplicate()"
    >
      <CopyPlus />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      :title="t('workflow.edit.copy')"
      :aria-label="t('workflow.edit.copy')"
      @click="props.commands.copy()"
    >
      <Copy />
    </Button>
    <Button
      variant="ghost"
      size="icon-sm"
      :title="t('workflow.edit.paste')"
      :aria-label="t('workflow.edit.paste')"
      @click="props.commands.paste()"
    >
      <ClipboardPaste />
    </Button>
  </fieldset>
</template>
