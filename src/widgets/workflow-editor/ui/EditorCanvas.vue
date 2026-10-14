<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { VueFlow } from '@vue-flow/core'
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'
import { MiniMap } from '@vue-flow/minimap'
import '@vue-flow/core/dist/style.css'
import '@vue-flow/core/dist/theme-default.css'
import '@vue-flow/controls/dist/style.css'
import '@vue-flow/minimap/dist/style.css'

import { WorkflowNodeCard, type WorkflowNodeData } from '@/entities/workflow'
import { Kbd } from '@/shared/ui/kbd'

import type { EditorNode, EditorNoteData } from '../model/flow'
import { useSpacePan } from '../model/space-pan'
import type { useCanvasInsert } from '../model/use-canvas-insert'
import type { EditorGraph } from '../model/use-editor-graph'
import { leaveFields } from '../model/use-editor-hotkeys'
import NoteCard from './NoteCard.vue'

/** The canvas: nodes, notes and wires; Ctrl+Space adds a node, hold Space to pan. */
const props = defineProps<{
  editor: EditorGraph
  insert: ReturnType<typeof useCanvasInsert>
  display: (id: string, data: EditorNode['data']) => WorkflowNodeData
  /** The workflow is loaded and has no nodes: the hint on how to start shows. */
  empty: boolean
}>()

const { t } = useI18n()
const { flow, nodes, edges } = props.editor
const spacePan = useSpacePan({ get: flow.getViewport, set: (next) => void flow.setViewport(next) })
</script>

<template>
  <section
    class="wf-canvas relative min-h-[28rem] flex-1 overflow-hidden rounded-lg border border-border bg-background"
    data-tour="workflow-canvas"
    :aria-label="t('workflow.canvasLabel')"
    :data-space-pan="spacePan.held.value ? (spacePan.panning.value ? 'grabbing' : 'grab') : undefined"
    @pointerdown.capture="spacePan.onPress"
    @mousedown.capture="spacePan.onPress"
    @pointerdown="leaveFields"
    @pointermove="insert.onPointerMove"
  >
    <VueFlow
      :id="editor.flowId"
      v-model:nodes="nodes"
      v-model:edges="edges"
      :is-valid-connection="editor.isValidConnection"
      :delete-key-code="null"
      :pan-activation-key-code="null"
      :min-zoom="0.2"
      :max-zoom="2"
      :snap-to-grid="true"
      :snap-grid="[10, 10]"
      fit-view-on-init
    >
      <template #node-wf="nodeProps">
        <WorkflowNodeCard :id="nodeProps.id" :data="display(nodeProps.id, nodeProps.data)" :selected="nodeProps.selected" />
      </template>
      <template #node-note="noteProps">
        <NoteCard
          :id="noteProps.id"
          :data="noteProps.data as EditorNoteData"
          :selected="noteProps.selected"
          @text="editor.noteText"
        />
      </template>
      <Background :gap="20" />
      <Controls :show-interactive="false" />
      <MiniMap pannable zoomable />
    </VueFlow>
    <div
      v-if="empty"
      class="pointer-events-none absolute inset-0 flex items-center justify-center p-8 text-center text-sm text-muted-foreground"
    >
      {{ t('workflow.canvas.empty') }}
    </div>
    <p
      data-tour="workflow-quick-hint"
      class="pointer-events-none absolute bottom-2 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-md bg-background/80 px-2 py-1 text-xs text-muted-foreground">
      <Kbd>Ctrl</Kbd>+<Kbd>Space</Kbd>
      {{ t('workflow.canvas.addHint') }}
    </p>
  </section>
</template>
