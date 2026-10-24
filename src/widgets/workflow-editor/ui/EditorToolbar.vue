<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { ChevronRight, ClipboardPaste, Copy, CopyPlus, FlaskConical, Keyboard, MoreHorizontal, Play, StickyNote, Undo2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'
import { Toggle } from '@/shared/ui/toggle'
import { pluralForm } from '@/shared/lib/format'

import type { TriggerChoice } from '../model/trigger-choices'
import type { EditCommandSet } from '../model/edit-commands'
import EditCommands from './EditCommands.vue'
import RunFromMenu from './RunFromMenu.vue'
import { activeStatus } from '../model/node-problem'

/**
 * The editor's top row (Auto-2 · 13): «Автоматизации ›», the name, on/off,
 * undo and redo, then the simple mode, the emulation and «Сохранить». The
 * rest — run now, duplicate, copy, paste, a note, the keys — sits under «⋯».
 */
const props = defineProps<{
  nameError?: string
  /** Shown under the row: a missing name or errors on a workflow switched on. */
  message?: string
  canRun: boolean
  canDiscard: boolean
  dirty: boolean
  /** Errors that keep the scheme off: the switch is locked until they are fixed. */
  blockers: number
  hasNodes: boolean
  emulating: boolean
  running: boolean
  saving: boolean
  /** Triggers the run buttons can start from, the default first. */
  triggers: TriggerChoice[]
  /** Undo, redo, duplicate, copy, paste. */
  commands: EditCommandSet
}>()
const emit = defineEmits<{
  note: []
  emulate: [triggerId: string | null]
  run: [triggerId: string | null]
  discard: []
  save: [event: Event]
}>()
const name = defineModel<string>('name', { required: true })
const active = defineModel<boolean>('active', { required: true })
const simple = defineModel<boolean>('simple', { required: true })
const hotkeysOpen = defineModel<boolean>('hotkeys', { required: true })

const { t, locale } = useI18n()
const status = computed(() => activeStatus(active.value, props.blockers, t, pluralForm(props.blockers, locale.value)))
</script>

<template>
  <div class="flex items-center gap-2" data-tour="workflow-toolbar">
    <RouterLink :to="{ name: 'automations' }" class="flex shrink-0 items-center gap-1 text-sm text-muted-foreground hover:text-foreground">
      {{ t('nav.automations') }}
      <ChevronRight class="size-3.5" />
    </RouterLink>
    <div class="w-full min-w-0 shrink sm:w-56" :data-invalid="nameError ? '' : undefined">
      <Input
        v-model="name"
        :aria-label="t('workflow.name')"
        :aria-invalid="Boolean(nameError)"
        :placeholder="t('workflow.name')"
        :class="nameError ? 'border-destructive' : ''"
      />
    </div>
    <div class="flex shrink-0 items-center gap-2" data-tour="workflow-active">
      <Switch id="workflow-active" v-model="active" :disabled="!active && blockers > 0" />
      <Label for="workflow-active" :class="!active && blockers ? 'text-warning' : ''" data-active-status>{{ status }}</Label>
    </div>
    <span class="flex-1" />
    <EditCommands compact :commands="commands" data-tour="workflow-edit-commands" />
    <ConfirmButton
      v-if="canDiscard"
      :label="t('workflow.discard.button')"
      :confirm-label="t('workflow.discard.confirm')"
      data-tour="workflow-discard"
      @confirm="emit('discard')"
    >
      <Undo2 class="size-4" />
    </ConfirmButton>
    <Toggle v-model="simple" variant="outline" class="shrink-0 px-3" data-tour="workflow-simple">{{ t('workflow.simpleMode') }}</Toggle>
    <DropdownMenu align="end">
      <template #trigger>
        <Button variant="ghost" size="icon" :aria-label="t('workflow.more')" :title="t('workflow.more')" data-tour="workflow-run">
          <MoreHorizontal class="size-4" />
        </Button>
      </template>
      <template v-if="canRun">
        <DropdownMenuItem v-for="choice in triggers" :key="choice.id" :value="choice.id" :disabled="running || dirty" @select="(id) => emit('run', id)">
          <Play />
          <span class="truncate">{{ triggers.length > 1 ? t('workflow.runFromTrigger', { trigger: choice.title }) : t('workflow.runNow') }}</span>
        </DropdownMenuItem>
        <p v-if="dirty" class="px-2 pb-1 text-xs text-muted-foreground">{{ t('workflow.saveFirst') }}</p>
      </template>
      <DropdownMenuItem value="duplicate" @select="commands.duplicate()"><CopyPlus />{{ t('workflow.edit.duplicate') }}</DropdownMenuItem>
      <DropdownMenuItem value="copy" @select="commands.copy()"><Copy />{{ t('workflow.edit.copy') }}</DropdownMenuItem>
      <DropdownMenuItem value="paste" @select="commands.paste()"><ClipboardPaste />{{ t('workflow.edit.paste') }}</DropdownMenuItem>
      <DropdownMenuItem value="note" @select="emit('note')"><StickyNote />{{ t('workflow.note.add') }}</DropdownMenuItem>
      <DropdownMenuItem value="keys" @select="hotkeysOpen = !hotkeysOpen"><Keyboard />{{ t('workflow.hotkeys') }}</DropdownMenuItem>
    </DropdownMenu>
    <RunFromMenu
      :label="t('workflow.emulate')"
      :icon="FlaskConical"
      :triggers="triggers"
      :disabled="emulating || !hasNodes"
      :title="hasNodes ? undefined : t('workflow.emulateNeedsNodes')"
      class="shrink-0"
      data-tour="workflow-emulate"
      @run="(id) => emit('emulate', id)"
    />
    <Button class="shrink-0" data-tour="workflow-save" :disabled="saving" @click="emit('save', $event)">
      {{ t('actions.save') }}
    </Button>
  </div>
  <p v-if="message" class="text-xs text-destructive" role="alert">{{ message }}</p>
</template>
