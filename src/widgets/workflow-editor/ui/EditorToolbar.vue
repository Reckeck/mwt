<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { FlaskConical, Keyboard, Play, Plus, Save, StickyNote, Undo2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'

/** The editor's top row: name, on/off, mode, and the actions on the whole workflow. */
defineProps<{
  nameError?: string
  /** Shown under the row: a missing name or errors on a workflow switched on. */
  message?: string
  canRun: boolean
  canDiscard: boolean
  dirty: boolean
  hasNodes: boolean
  emulating: boolean
  running: boolean
  saving: boolean
}>()
const emit = defineEmits<{
  note: []
  quickAdd: []
  emulate: []
  run: []
  discard: []
  save: [event: Event]
}>()
const name = defineModel<string>('name', { required: true })
const active = defineModel<boolean>('active', { required: true })
const simple = defineModel<boolean>('simple', { required: true })
const hotkeysOpen = defineModel<boolean>('hotkeys', { required: true })

const { t } = useI18n()
</script>

<template>
  <div class="flex flex-wrap items-center gap-2" data-tour="workflow-toolbar">
    <div class="w-64" :data-invalid="nameError ? '' : undefined">
      <Input
        v-model="name"
        :aria-label="t('workflow.name')"
        :aria-invalid="Boolean(nameError)"
        :placeholder="t('workflow.name')"
        :class="nameError ? 'border-destructive' : ''"
      />
    </div>
    <div class="flex items-center gap-2" data-tour="workflow-active">
      <Switch id="workflow-active" v-model="active" />
      <Label for="workflow-active">{{ active ? t('workflow.active') : t('workflow.inactive') }}</Label>
    </div>
    <span class="flex-1" />
    <div class="flex items-center gap-2">
      <Switch id="workflow-simple" v-model="simple" />
      <Label for="workflow-simple" class="text-sm">{{ t('workflow.simpleMode') }}</Label>
    </div>
    <Button
      variant="ghost"
      size="icon"
      :aria-label="t('workflow.hotkeys')"
      :aria-pressed="hotkeysOpen"
      @click="hotkeysOpen = !hotkeysOpen"
    >
      <Keyboard class="size-4" />
    </Button>
    <Button variant="ghost" size="icon" :aria-label="t('workflow.note.add')" :title="t('workflow.note.add')" @click="emit('note')">
      <StickyNote class="size-4" />
    </Button>
    <Button variant="outline" data-tour="workflow-quick-button" @click="emit('quickAdd')">
      <Plus class="size-4" />
      {{ t('workflow.addNode') }}
    </Button>
    <Button variant="outline" data-tour="workflow-emulate" :disabled="emulating || !hasNodes" @click="emit('emulate')">
      <FlaskConical class="size-4" />
      {{ t('workflow.emulate') }}
    </Button>
    <Button
      v-if="canRun"
      variant="outline"
      :disabled="running || dirty"
      :title="dirty ? t('workflow.saveFirst') : undefined"
      @click="emit('run')"
    >
      <Play class="size-4" />
      {{ t('workflow.runNow') }}
    </Button>
    <ConfirmButton
      v-if="canDiscard"
      :label="t('workflow.discard.button')"
      :confirm-label="t('workflow.discard.confirm')"
      data-tour="workflow-discard"
      @confirm="emit('discard')"
    >
      <Undo2 class="size-4" />
    </ConfirmButton>
    <Button data-tour="workflow-save" :disabled="saving" @click="emit('save', $event)">
      <Save class="size-4" />
      {{ t('actions.save') }}
    </Button>
  </div>
  <p v-if="message" class="text-xs text-destructive" role="alert">{{ message }}</p>
</template>
