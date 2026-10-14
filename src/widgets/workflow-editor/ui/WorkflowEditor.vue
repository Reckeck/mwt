<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useLocalStorage } from '@vueuse/core'

import { type WorkflowNodeData, useNodeCatalog } from '@/entities/workflow'
import { useWorkflowActions } from '@/features/workflow-manage'
import { QuickAddPanel } from '@/features/workflow-quick-add'
import { required, useFormValidation } from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'

import { nodeSummary } from '../model/fields'
import type { EditorNode } from '../model/flow'
import { useCanvasInsert } from '../model/use-canvas-insert'
import { useEditorGraph } from '../model/use-editor-graph'
import { useEditorHotkeys } from '../model/use-editor-hotkeys'
import { useGraphClipboard } from '../model/use-graph-clipboard'
import { useGraphValidation } from '../model/use-graph-validation'
import { triggerChoices } from '../model/trigger-choices'
import { useRedundantFlows } from '../model/use-redundant-flows'
import { useRunReport } from '../model/use-run-report'
import { useSelection } from '../model/use-selection'
import { useWorkflowDocument } from '../model/use-workflow-document'
import type { WorkflowEditorProps } from '../workflow-editor.types'
import DraftBanner from './DraftBanner.vue'
import EditorCanvas from './EditorCanvas.vue'
import EditorToolbar from './EditorToolbar.vue'
import HotkeysHelp from './HotkeysHelp.vue'
import LessonPanel from './LessonPanel.vue'
import NodeInspector from './NodeInspector.vue'
import RunPanel from './RunPanel.vue'
import WorkflowHistoryPanel from './WorkflowHistoryPanel.vue'
import WorkflowSettingsPanel from './WorkflowSettingsPanel.vue'

/**
 * The node editor: the canvas and the inspector on the right. Nodes are
 * added only with Ctrl+Space (or Shift+A): a search of the nodes that fit where
 * the cursor is; the emulation
 * shows the run step by step and the value on every wire. The logic lives in
 * `model/use-*`; this file wires it to the layout.
 */
const props = withDefaults(defineProps<WorkflowEditorProps>(), { workflowId: null, template: null, lesson: null })
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
const { name, description, active, react, maxAmount, lesson, dirty } = doc
const insert = useCanvasInsert(editor)
const { quick } = insert
const clipboard = useGraphClipboard(editor)
const { selectedNode, selectedDef, wiredFrom, flowLinks, dataAfter, setParam, setLabel, unwire } = useSelection(editor)

/** An empty flow port in the inspector: the node search, joined to that port. */
function addFlow(port: string, side: 'in' | 'out', event: MouseEvent) {
  if (selectedNode.value) insert.quickAddFrom(selectedNode.value.id, port, side, event)
}

const triggers = computed(() => triggerChoices(nodes.value, editor.def, locale.value))

const helpOpen = ref(false)
const hotkeysOpen = ref(false)
const editorRoot = ref<HTMLElement | null>(null)

/** Save stays enabled; a missing name or errors on a workflow switched on show under the toolbar. */
const form = useFormValidation(() => ({
  name: required(name.value),
  graph: active.value && errors.value.length ? t('workflow.missing.errors') : null,
}))
const save = () => doc.save((id) => emit('saved', id))

useEditorHotkeys(editorRoot, () => Boolean(quick.value), {
  save: (event) => form.submit(save, event),
  emulate: () => run.emulate(),
  toggleHelp: () => (helpOpen.value = !helpOpen.value),
  quickAdd: insert.quickAddHere,
  undo: doc.history.undo,
  redo: doc.history.redo,
  duplicate: clipboard.duplicate,
  copy: clipboard.copySelected,
  paste: () => clipboard.paste(),
  toggleSelected: editor.toggleSelected,
  selectAll: () => flow.addSelectedNodes(nodes.value as never),
  fit: () => void flow.fitView({ padding: 0.2, duration: 300 }),
  remove: editor.removeSelected,
})

function display(id: string, data: EditorNode['data']): WorkflowNodeData {
  const nodeDef = editor.def(data?.kind ?? '')
  return {
    def: nodeDef,
    kind: data?.kind ?? '',
    label: data?.label ?? '',
    disabled: data?.disabled ?? false,
    issues: issuesOf(id),
    summary: nodeSummary(nodeDef, data?.params ?? {}, locale.value),
    step: run.stepOf.value.get(id) ?? null,
  }
}

/** A lesson is practice: leaving it never asks to save. */
defineExpose({
  dirty: computed(() => dirty.value && !lesson.value),
  canDraft: computed(() => Boolean(props.workflowId)),
  saveDraft: doc.saveDraft,
  discardEdits: doc.discardEdits,
})
</script>

<template>
  <div
    class="flex h-full min-h-0 flex-col gap-3"
    ref="editorRoot"
    role="application"
    :aria-label="t('workflow.editorLabel')"
    tabindex="-1"
  >
    <EditorToolbar
      v-model:name="name"
      v-model:active="active"
      v-model:simple="simple"
      v-model:hotkeys="hotkeysOpen"
      :name-error="form.error('name')"
      :message="form.firstMessage()"
      :can-run="Boolean(props.workflowId)"
      :can-discard="dirty && !lesson"
      :dirty="dirty"
      :has-nodes="nodes.length > 0"
      :emulating="actions.dryRun.isPending.value"
      :running="actions.runNow.isPending.value"
      :saving="actions.save.isPending.value"
      :triggers="triggers"
      @note="editor.addNote"
      @emulate="(id) => run.emulate(false, id)"
      @run="run.runNow"
      @discard="doc.discardEdits"
      @save="(event) => form.submit(save, event)"
    />
    <DraftBanner
      v-if="doc.storedDraft.value"
      :saved-at="doc.storedDraft.value.saved_at"
      :dropping="actions.discardDraft.isPending.value"
      @open="doc.openDraft"
      @drop="doc.dropDraft"
    />

    <div v-if="hotkeysOpen" class="rounded-lg border border-border bg-card p-3"><HotkeysHelp /></div>

    <div class="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_17rem] gap-3 xl:grid-cols-[minmax(0,1fr)_20rem]">

      <div class="flex min-h-0 flex-col gap-3">
        <EditorCanvas :editor="editor" :insert="insert" :display="display" :empty="doc.loaded.value && !nodes.length" />
        <RunPanel
          v-if="report"
          :report="report"
          :answers="answers"
          :can-save-test="Boolean(props.workflowId) && report.run.dry_run && !dirty"
          @focus="editor.focusNode"
          @answer="run.answer"
          @close="report = null"
          @save-test="run.saveAsTest"
        />
      </div>

      <div class="flex min-h-0 min-w-0 flex-col gap-3 overflow-x-hidden overflow-y-auto">
        <LessonPanel
          v-if="lesson"
          :lesson="lesson"
          :graph="editor.graph.value"
          :report="report"
          :checking="actions.dryRun.isPending.value"
          @check="run.emulate()"
          @next="(id) => emit('lesson', id)"
        />
        <div class="rounded-lg border border-border bg-card p-3">
          <NodeInspector
            v-if="selectedNode"
            v-model:help="helpOpen"
            :node="selectedNode"
            :def="selectedDef"
            :wired-from="wiredFrom"
            :flows="flowLinks"
            :data-after="dataAfter"
            :issues="issuesOf(selectedNode.id)"
            @param="setParam"
            @label="setLabel"
            @toggle="editor.toggleSelected"
            @unwire="unwire"
            @unlink="editor.removeEdge"
            @add-flow="addFlow"
            @remove="editor.removeSelected"
          />
          <WorkflowSettingsPanel
            v-else
            v-model:description="description"
            v-model:max-amount="maxAmount"
            v-model:react="react"
            :issues="issues"
            :redundant="redundant.count.value"
            :base="settings.baseCurrency"
            @focus="editor.focusNode"
            @drop-redundant="redundant.removeAll"
          />
          <WorkflowHistoryPanel
            v-if="!selectedNode && doc.workflow.data.value"
            class="mt-4 border-t border-border pt-4"
            :workflow="doc.workflow.data.value"
            :dirty="dirty"
            @restored="doc.onRestored"
          />
        </div>
      </div>
    </div>

    <QuickAddPanel
      v-if="quick"
      :left="quick.left"
      :top="quick.top"
      :context="quick.context"
      :simple="simple"
      @pick="insert.onQuickPick"
      @close="quick = null"
    />
  </div>
</template>
