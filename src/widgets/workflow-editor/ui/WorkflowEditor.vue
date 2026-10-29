<script setup lang="ts">
// check-ui-ignore grid-fixed-track: the canvas + side panel grid is the desktop branch; a phone gets MobileInspector
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useLocalStorage } from '@vueuse/core'

import { useNodeCatalog } from '@/entities/workflow'
import { useWorkflowActions } from '@/features/workflow-manage'
import { required, useFormValidation } from '@/shared/lib/form'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'
import { useSettingsStore } from '@/shared/store/settings'

import { useCanvasInsert } from '../model/use-canvas-insert'
import { useEditorGraph } from '../model/use-editor-graph'
import { useEditorHotkeys } from '../model/use-editor-hotkeys'
import { useGraphClipboard } from '../model/use-graph-clipboard'
import { useGraphValidation } from '../model/use-graph-validation'
import { triggerChoices } from '../model/trigger-choices'
import { useRedundantFlows } from '../model/use-redundant-flows'
import { useRunReport } from '../model/use-run-report'
import { useSchemeWords } from '../model/use-scheme-words'
import { useWorkflowDocument } from '../model/use-workflow-document'
import type { WorkflowEditorProps } from '../workflow-editor.types'
import { useMobileSheet } from '../model/use-mobile-sheet'
import { useNodeDisplay } from '../model/use-node-display'
import DraftBanner from './DraftBanner.vue'
import EditorCanvas from './EditorCanvas.vue'
import EditorSidePanel from './EditorSidePanel.vue'
import type { EditCommandSet } from '../model/edit-commands'
import EditorToolbar from './EditorToolbar.vue'
import HotkeysHelp from './HotkeysHelp.vue'
import MobileEditorBar from './MobileEditorBar.vue'
import MobileInspector from './MobileInspector.vue'
import MobileRunCard from './MobileRunCard.vue'
import MobileSteps from './MobileSteps.vue'
import QuickAddHost from './QuickAddHost.vue'
import RunPanel from './RunPanel.vue'
import SchemeWords from './SchemeWords.vue'

/**
 * The node editor: the canvas and the inspector on the right; a phone gets the
 * steps as a list, the canvas a tap away. Nodes are added with Ctrl+Space (or
 * Shift+A); the emulation shows the run step by step and the value on every
 * wire. The logic lives in `model/use-*`; this file wires it to the layout.
 */
const props = withDefaults(defineProps<WorkflowEditorProps>(), { workflowId: null, template: null, lesson: null, locked: false })
const emit = defineEmits<{ saved: [id: string]; lesson: [id: string] }>()

const { t, locale } = useI18n()
const settings = useSettingsStore()
const catalog = useNodeCatalog()
const actions = useWorkflowActions()

const editor = useEditorGraph(catalog)
const { flow, nodes } = editor
const simple = useLocalStorage('workflow.editor.simple', true)
const { issues, issuesOf, errors } = useGraphValidation(editor.graphJson)
const redundant = useRedundantFlows(editor, issues)
const run = useRunReport(editor, actions, () => props.workflowId)
const { report, answers } = run
const doc = useWorkflowDocument(editor, catalog.query, actions, report, {
  workflowId: () => props.workflowId,
  template: () => props.template,
  lesson: () => props.lesson,
})
const { name, active, lesson, dirty } = doc
const insert = useCanvasInsert(editor)
const { quick } = insert
const clipboard = useGraphClipboard(editor)
/** What the side panel (or the phone's sheet) needs. */
const panel = computed(() => ({
  editor,
  insert,
  doc,
  run,
  issues: issues.value,
  issuesOf,
  redundant,
  checking: actions.dryRun.isPending.value,
  base: settings.baseCurrency,
}))

const scheme = useSchemeWords(editor)
const display = useNodeDisplay(editor, issuesOf, scheme, run)
const triggers = computed(() => triggerChoices(nodes.value, editor.def, locale.value))

const helpOpen = ref(false)
const hotkeysOpen = ref(false)
const editorRoot = ref<HTMLElement | null>(null)

/** Save stays enabled; a missing name or errors on a workflow switched on show under the toolbar. */
const form = useFormValidation(() => ({
  name: required(name.value),
  graph: active.value && errors.value.length ? t('workflow.missing.errors') : null,
}))
const save = () => (props.locked ? undefined : doc.save((id) => emit('saved', id)))

/** A phone: one row on top, the canvas on the whole screen, the panels in a sheet. */
const phone = useIsPhone()
const sheet = useMobileSheet(editor, report, phone)
/** The phone shows the steps; `Canvas` swaps in the canvas, kept mounted so the selection lives on. */
const showCanvas = ref(false)
// Hidden, the nodes had no size: fit once the canvas has been measured.
watch(showCanvas, (shown) => shown && void nextTick(() => setTimeout(() => flow.fitView({ padding: 0.2 }), 120)))
const barState = computed(() => ({
  nameError: form.error('name'),
  message: form.firstMessage(),
  canRun: Boolean(props.workflowId),
  canDiscard: dirty.value && !lesson.value,
  dirty: dirty.value,
  blockers: errors.value.length,
  hasNodes: nodes.value.length > 0,
  emulating: actions.dryRun.isPending.value,
  running: actions.runNow.isPending.value,
  saving: actions.save.isPending.value || props.locked,
}))
const runProps = computed(() => ({
  report: report.value as NonNullable<typeof report.value>,
  answers: answers.value,
  canSaveTest: Boolean(props.workflowId) && Boolean(report.value?.run.dry_run) && !dirty.value,
}))

const commands: EditCommandSet = { ...doc.history, copy: clipboard.copySelected, duplicate: clipboard.duplicate, paste: () => clipboard.paste() }
useEditorHotkeys(editorRoot, () => Boolean(quick.value), {
  ...commands,
  save: (event) => form.submit(save, event),
  emulate: () => run.emulate(),
  toggleHelp: () => (helpOpen.value = !helpOpen.value),
  quickAdd: insert.quickAddHere,
  toggleSelected: editor.toggleSelected,
  selectAll: () => flow.addSelectedNodes(nodes.value as never),
  fit: () => void flow.fitView({ padding: 0.2, duration: 300 }),
  remove: editor.removeSelected,
})

/** A lesson is practice: leaving it never asks to save. */
defineExpose({
  /** For the page's breadcrumbs. */
  name: computed(() => doc.name.value.trim()),
  dirty: computed(() => dirty.value && !lesson.value),
  canDraft: computed(() => Boolean(props.workflowId) && !props.locked),
  saveDraft: doc.saveDraft,
  discardEdits: doc.discardEdits,
})
</script>

<template>
  <div
    :class="cn('flex h-full min-h-0 flex-col', !phone && 'gap-3')"
    ref="editorRoot"
    role="application"
    :aria-label="t('workflow.editorLabel')"
    tabindex="-1"
  >
    <MobileEditorBar
      v-if="phone"
      v-model:name="name"
      v-model:active="active"
      v-model:simple="simple"
      v-bind="barState"
      :commands="commands"
      @note="editor.addNote"
      @paste="clipboard.paste()"
      @panel="sheet.show('inspector')"
      @emulate="run.emulate(false, triggers[0]?.id ?? null)"
      @run="run.runNow(triggers[0]?.id ?? null)"
      @discard="doc.discardEdits"
      @save="(event) => form.submit(save, event)"
    />
    <EditorToolbar
      v-else
      v-model:name="name"
      v-model:active="active"
      v-model:simple="simple"
      v-model:hotkeys="hotkeysOpen"
      v-bind="barState"
      :triggers="triggers"
      :commands="commands"
      @note="editor.addNote"
      @emulate="(id) => run.emulate(false, id)"
      @run="run.runNow"
      @discard="doc.discardEdits"
      @save="(event) => form.submit(save, event)"
    />
    <SchemeWords v-if="!phone" :parts="scheme.words.value" />
    <DraftBanner
      v-if="doc.storedDraft.value"
      :saved-at="doc.storedDraft.value.saved_at"
      :dropping="actions.discardDraft.isPending.value"
      @open="doc.openDraft"
      @drop="doc.dropDraft"
    />

    <div v-if="hotkeysOpen && !phone" class="rounded-lg border border-border bg-card p-3"><HotkeysHelp /></div>

    <template v-if="phone">
      <MobileSteps v-model:canvas="showCanvas" class="px-4 pt-3" :editor="editor" :scheme="scheme" :issues-of="issuesOf" :order="run.orderOf.value" @add="insert.quickAddCentre">
        <MobileRunCard v-if="report" :report="report" :answers="run.answers.value" @answer="run.answer" @details="sheet.show('run')" />
      </MobileSteps>
      <EditorCanvas v-show="showCanvas" touch :editor="editor" :insert="insert" :display="display" :empty="doc.loaded.value && !nodes.length" />
      <MobileInspector
        v-model:open="sheet.open.value"
        v-model:tab="sheet.tab.value"
        :has-run="Boolean(report)"
        :has-lesson="Boolean(lesson)"
        :node-selected="editor.flow.getSelectedNodes.value.length === 1"
      >
        <template #inspector><EditorSidePanel v-model:help="helpOpen" part="inspector" v-bind="panel" /></template>
        <template #run>
        <RunPanel
          v-if="report"
          v-bind="runProps"
          @focus="editor.focusNode"
          @answer="run.answer"
          @close="report = null"
          @save-test="run.saveAsTest"
        />
      </template>
        <template #lesson><EditorSidePanel v-model:help="helpOpen" part="lesson" v-bind="panel" @lesson="(id) => emit('lesson', id)" /></template>
      </MobileInspector>
    </template>
    <div v-else class="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)] grid-cols-[minmax(0,1fr)_17rem] gap-3 xl:grid-cols-[minmax(0,1fr)_18rem]">
      <div class="flex min-h-0 flex-col gap-3">
        <EditorCanvas :editor="editor" :insert="insert" :display="display" :empty="doc.loaded.value && !nodes.length" />
        <RunPanel
          v-if="report"
          v-bind="runProps"
          @focus="editor.focusNode"
          @answer="run.answer"
          @close="report = null"
          @save-test="run.saveAsTest"
        />
      </div>

      <EditorSidePanel
        v-model:help="helpOpen"
        class="overflow-x-hidden overflow-y-auto"
        v-bind="panel"
        @lesson="(id) => emit('lesson', id)"
      />
    </div>

    <QuickAddHost :state="quick" :simple="simple" :phone="phone" @pick="insert.onQuickPick" @close="quick = null" />
  </div>
</template>
