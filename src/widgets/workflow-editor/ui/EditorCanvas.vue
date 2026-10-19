<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { VueFlow } from '@vue-flow/core'
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'
import { MiniMap } from '@vue-flow/minimap'
import { Plus } from 'lucide-vue-next'
import '@vue-flow/core/dist/style.css'
import '@vue-flow/core/dist/theme-default.css'
import '@vue-flow/controls/dist/style.css'
import '@vue-flow/minimap/dist/style.css'

import { WorkflowNodeCard, type WorkflowNodeData } from '@/entities/workflow'
import { Button } from '@/shared/ui/button'
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
  /** A phone: the canvas is the whole screen, nodes come from the «+» button, no mini-map or key hints. */
  touch?: boolean
}>()

const { t } = useI18n()
const { flow, nodes, edges } = props.editor
const spacePan = useSpacePan({ get: flow.getViewport, set: (next) => void flow.setViewport(next) })

/** A small graph is seen whole: the mini-map would only cover part of it. */
const MINIMAP_FROM_NODES = 6

/** A double or right click on the empty canvas opens the node search right there. */
function addHere(event: MouseEvent) {
  event.preventDefault()
  props.insert.quickAddHere()
}
function onCanvasDblClick(event: MouseEvent) {
  if ((event.target as HTMLElement | null)?.closest('.vue-flow__pane')) addHere(event)
}
</script>

<template>
  <section
    :class="props.touch ? 'min-h-0 rounded-none border-0' : 'min-h-[28rem] rounded-lg border border-border'"
    class="wf-canvas relative flex-1 overflow-hidden bg-background"
    data-tour="workflow-canvas"
    :aria-label="t('workflow.canvasLabel')"
    :data-space-pan="spacePan.held.value ? (spacePan.panning.value ? 'grabbing' : 'grab') : undefined"
    @pointerdown.capture="spacePan.onPress"
    @mousedown.capture="spacePan.onPress"
    @pointerdown="leaveFields"
    @pointermove="insert.onPointerMove"
    @dblclick="onCanvasDblClick"
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
      :zoom-on-double-click="false"
      connect-on-click
      fit-view-on-init
      @pane-context-menu="addHere"
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
      <MiniMap v-if="!props.touch && nodes.length >= MINIMAP_FROM_NODES" pannable zoomable />
    </VueFlow>
    <div
      v-if="empty"
      class="pointer-events-none absolute inset-0 flex flex-col items-center justify-center gap-3 p-8 text-center text-sm text-muted-foreground"
    >
      {{ props.touch ? t('workflow.canvas.emptyTouch') : t('workflow.canvas.empty') }}
      <Button class="pointer-events-auto" data-tour="workflow-add-first" @click="props.touch ? insert.quickAddCentre() : insert.quickAddHere()">
        {{ t('workflow.canvas.addFirst') }}
      </Button>
    </div>
    <Button
      v-if="props.touch && !empty"
      size="icon"
      class="absolute right-3 bottom-3 size-12 rounded-full shadow-lg"
      :aria-label="t('workflow.mobile.add')"
      data-tour="workflow-add-touch"
      @click="insert.quickAddCentre()"
    >
      <Plus class="size-5" />
    </Button>
    <p
      v-if="!props.touch"
      data-tour="workflow-quick-hint"
      class="pointer-events-none absolute bottom-2 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-md bg-background/80 px-2 py-1 text-xs text-muted-foreground">
      <Kbd>Ctrl</Kbd>+<Kbd>Space</Kbd>
      {{ t('workflow.canvas.addHint') }}
    </p>
  </section>
</template>
