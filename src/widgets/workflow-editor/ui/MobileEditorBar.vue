<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowLeft,
  ClipboardPaste,
  Copy,
  CopyPlus,
  FlaskConical,
  MoreVertical,
  PanelBottom,
  Play,
  Redo2,
  Save,
  StickyNote,
  Undo2,
} from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'
import type { EditCommandSet } from '../model/edit-commands'

/**
 * The phone editor's one row: back, the name (a tap renames it and switches
 * it on or off), check, save, and the rest under «⋯». Checking and running
 * start from the default trigger.
 */
const props = defineProps<{
  nameError?: string
  message?: string
  canRun: boolean
  canDiscard: boolean
  dirty: boolean
  hasNodes: boolean
  emulating: boolean
  running: boolean
  saving: boolean
  /** Undo, redo, duplicate, copy (paste has its own item). */
  commands: EditCommandSet
}>()
const emit = defineEmits<{
  note: []
  paste: []
  panel: []
  emulate: []
  run: []
  discard: []
  save: [event: Event]
}>()
const name = defineModel<string>('name', { required: true })
const active = defineModel<boolean>('active', { required: true })
const simple = defineModel<boolean>('simple', { required: true })

const { t } = useI18n()
const router = useRouter()
const renameOpen = ref(false)

function back() {
  if (window.history.length > 1) router.back()
  else void router.push({ name: 'automations-workflows' })
}
</script>

<template>
  <div class="flex items-center gap-1 border-b border-border px-1 py-1.5" data-tour="workflow-toolbar">
    <Button variant="ghost" size="icon" :aria-label="t('actions.back')" @click="back"><ArrowLeft class="size-4" /></Button>
    <Button
      variant="ghost"
      class="h-auto min-w-0 flex-1 flex-col items-start gap-0 px-1 py-0.5 text-left font-normal"
      :aria-label="t('workflow.mobile.rename')"
      @click="renameOpen = true"
    >
      <span class="w-full truncate text-sm font-medium" :class="props.nameError && 'text-destructive'">
        {{ name || t('workflow.name') }}
      </span>
      <span class="text-xs text-muted-foreground">{{ active ? t('workflow.active') : t('workflow.inactive') }}</span>
    </Button>
    <Button
      variant="ghost"
      size="icon"
      :aria-label="t('workflow.emulate')"
      :disabled="props.emulating || !props.hasNodes"
      data-tour="workflow-emulate"
      @click="emit('emulate')"
    >
      <FlaskConical class="size-4" />
    </Button>
    <Button size="sm" :disabled="props.saving" data-tour="workflow-save" @click="emit('save', $event)">
      <Save class="size-4" />
      {{ t('actions.save') }}
    </Button>
    <DropdownMenu align="end">
      <template #trigger>
        <Button variant="ghost" size="icon" :aria-label="t('actions.more')"><MoreVertical class="size-4" /></Button>
      </template>
      <DropdownMenuItem value="panel" @select="emit('panel')">
        <PanelBottom class="size-4 shrink-0" />
        {{ t('workflow.mobile.panel') }}
      </DropdownMenuItem>
      <DropdownMenuItem v-if="props.canRun" value="run" :disabled="props.running || props.dirty" @select="emit('run')">
        <Play class="size-4 shrink-0" />
        {{ props.dirty ? t('workflow.saveFirst') : t('workflow.runNow') }}
      </DropdownMenuItem>
      <DropdownMenuItem value="note" @select="emit('note')">
        <StickyNote class="size-4 shrink-0" />
        {{ t('workflow.note.add') }}
      </DropdownMenuItem>
      <DropdownMenuItem value="undo" :disabled="!props.commands.canUndo()" @select="props.commands.undo()">
        <Undo2 class="size-4 shrink-0" />
        {{ t('workflow.edit.undo') }}
      </DropdownMenuItem>
      <DropdownMenuItem value="redo" :disabled="!props.commands.canRedo()" @select="props.commands.redo()">
        <Redo2 class="size-4 shrink-0" />
        {{ t('workflow.edit.redo') }}
      </DropdownMenuItem>
      <DropdownMenuItem value="duplicate" @select="props.commands.duplicate()">
        <CopyPlus class="size-4 shrink-0" />
        {{ t('workflow.edit.duplicate') }}
      </DropdownMenuItem>
      <DropdownMenuItem value="copy" @select="props.commands.copy()">
        <Copy class="size-4 shrink-0" />
        {{ t('workflow.edit.copy') }}
      </DropdownMenuItem>
      <DropdownMenuItem value="paste" @select="emit('paste')">
        <ClipboardPaste class="size-4 shrink-0" />
        {{ t('workflow.mobile.paste') }}
      </DropdownMenuItem>
      <DropdownMenuItem v-if="props.canDiscard" value="discard" destructive @select="emit('discard')">
        <Undo2 class="size-4 shrink-0" />
        {{ t('workflow.discard.button') }}
      </DropdownMenuItem>
    </DropdownMenu>
  </div>
  <p v-if="props.message" class="px-3 pt-1 text-xs text-destructive" role="alert">{{ props.message }}</p>

  <Dialog v-model:open="renameOpen" :title="t('workflow.mobile.rename')" size="sm">
    <div class="grid gap-4">
      <div class="grid gap-1.5" :data-invalid="props.nameError ? '' : undefined">
        <Label for="workflow-name-mobile">{{ t('workflow.name') }}</Label>
        <Input id="workflow-name-mobile" v-model="name" :aria-invalid="Boolean(props.nameError)" />
        <p v-if="props.nameError" class="text-xs text-destructive">{{ props.nameError }}</p>
      </div>
      <label for="workflow-active-mobile" class="flex items-center justify-between gap-3 text-sm" data-tour="workflow-active">
        {{ active ? t('workflow.active') : t('workflow.inactive') }}
        <Switch id="workflow-active-mobile" v-model="active" />
      </label>
      <label for="workflow-simple-mobile" class="flex items-center justify-between gap-3 text-sm">
        {{ t('workflow.simpleMode') }}
        <Switch id="workflow-simple-mobile" v-model="simple" />
      </label>
    </div>
    <template #footer>
      <Button @click="renameOpen = false">{{ t('ui.done') }}</Button>
    </template>
  </Dialog>
</template>
