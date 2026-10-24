<script setup lang="ts">
import type { useCanvasInsert } from '../model/use-canvas-insert'
import type { EditorGraph } from '../model/use-editor-graph'
import type { useRedundantFlows } from '../model/use-redundant-flows'
import type { useRunReport } from '../model/use-run-report'
import { useSelection } from '../model/use-selection'
import type { useWorkflowDocument } from '../model/use-workflow-document'
import LessonPanel from './LessonPanel.vue'
import NodeInspector from './NodeInspector.vue'
import WorkflowHistoryPanel from './WorkflowHistoryPanel.vue'
import WorkflowSettingsPanel from './WorkflowSettingsPanel.vue'

/**
 * Beside the canvas: the lesson's task, then the selected node's inspector or,
 * with nothing selected, the workflow's settings and history. A phone shows
 * the same in a sheet, one `part` per tab.
 */
const props = withDefaults(
  defineProps<{
    editor: EditorGraph
    insert: ReturnType<typeof useCanvasInsert>
    doc: ReturnType<typeof useWorkflowDocument>
    run: ReturnType<typeof useRunReport>
    issues: GraphIssue[]
    issuesOf: (id: string) => GraphIssue[]
    redundant: ReturnType<typeof useRedundantFlows>
    checking: boolean
    base: string
    part?: 'all' | 'inspector' | 'lesson'
  }>(),
  { part: 'all' },
)
const emit = defineEmits<{ lesson: [id: string] }>()
const help = defineModel<boolean>('help', { required: true })

const { selectedNode, selectedDef, wiredFrom, flowLinks, dataAfter, runsWhen, setParam, setLabel, unwire } = useSelection(props.editor)
const { lesson, workflow, description, maxAmount, react, dirty } = props.doc
const { report } = props.run

/** An empty flow port in the inspector: the node search, joined to that port. */
function addFlow(port: string, side: 'in' | 'out', event: MouseEvent) {
  if (selectedNode.value) props.insert.quickAddFrom(selectedNode.value.id, port, side, event)
}
</script>

<template>
  <div class="flex min-h-0 min-w-0 flex-col gap-3">
    <LessonPanel
      v-if="lesson && props.part !== 'inspector'"
      :lesson="lesson"
      :graph="props.editor.graph.value"
      :report="report"
      :checking="props.checking"
      @check="props.run.emulate()"
      @next="(id) => emit('lesson', id)"
    />
    <div v-if="props.part !== 'lesson'" class="rounded-lg border border-border bg-card p-3">
      <NodeInspector
        v-if="selectedNode"
        v-model:help="help"
        :node="selectedNode"
        :def="selectedDef"
        :wired-from="wiredFrom"
        :flows="flowLinks"
        :data-after="dataAfter"
        :runs-when="runsWhen"
        :issues="props.issuesOf(selectedNode.id)"
        @param="setParam"
        @label="setLabel"
        @toggle="props.editor.toggleSelected"
        @unwire="unwire"
        @unlink="props.editor.removeEdge"
        @add-flow="addFlow"
        @remove="props.editor.removeSelected"
      />
      <WorkflowSettingsPanel
        v-else
        v-model:description="description"
        v-model:max-amount="maxAmount"
        v-model:react="react"
        :issues="props.issues"
        :redundant="props.redundant.count.value"
        :base="props.base"
        @focus="props.editor.focusNode"
        @drop-redundant="props.redundant.removeAll"
      />
      <WorkflowHistoryPanel
        v-if="!selectedNode && workflow.data.value"
        class="mt-4 border-t border-border pt-4"
        :workflow="workflow.data.value"
        :dirty="dirty"
        @restored="props.doc.onRestored"
      />
    </div>
  </div>
</template>
