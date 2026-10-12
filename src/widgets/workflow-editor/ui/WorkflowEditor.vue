<script setup lang="ts">
import { type Ref, computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { type Connection, VueFlow, useVueFlow } from '@vue-flow/core'
import { Background } from '@vue-flow/background'
import { Controls } from '@vue-flow/controls'
import { MiniMap } from '@vue-flow/minimap'
import { FlaskConical, Keyboard, Play, Plus, Save, StickyNote } from 'lucide-vue-next'
import '@vue-flow/core/dist/style.css'
import '@vue-flow/core/dist/theme-default.css'
import '@vue-flow/controls/dist/style.css'
import '@vue-flow/minimap/dist/style.css'

import {
  WORKFLOW_LESSONS,
  WORKFLOW_TEMPLATES,
  WorkflowNodeCard,
  type WorkflowNodeData,
  useNodeCatalog,
} from '@/entities/workflow'
import { useWorkflowActions } from '@/features/workflow-manage'
import { type QuickAddChoice, type QuickAddContext, QuickAddPanel } from '@/features/workflow-quick-add'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'
import { call } from '@/shared/api/ipc'
import { required, useFormValidation } from '@/shared/lib/form'
import { useSettingsStore } from '@/shared/store/settings'
import {
  WORKFLOW_KEYS,
  emptyGraph,
  localized,
  outcomeKey,
  parseGraph,
  portAccepts,
  portType,
  shortId,
} from '@/shared/lib/workflow'

import { nodeSummary } from '../model/fields'
import {
  type EditorEdge,
  type EditorNode,
  type EditorNoteData,
  NOTE_PREFIX,
  isNote,
  toFlow,
  toFlowEdge,
  toGraph,
} from '../model/flow'
import { useHistory } from '../model/history'
import { isTyping } from '../model/hotkeys'
import { useSpacePan } from '../model/space-pan'
import type { WorkflowEditorProps } from '../workflow-editor.types'
import HotkeysHelp from './HotkeysHelp.vue'
import LessonPanel from './LessonPanel.vue'
import NodeInspector from './NodeInspector.vue'
import NodePalette from './NodePalette.vue'
import NoteCard from './NoteCard.vue'
import WorkflowHistoryPanel from './WorkflowHistoryPanel.vue'
import RunPanel from './RunPanel.vue'
import WorkflowSettingsPanel from './WorkflowSettingsPanel.vue'

/**
 * The node editor: a palette on the left, the canvas, the inspector on the
 * right. Ctrl+Space adds a node that fits where the cursor is; the emulation
 * shows the run step by step and the value on every wire.
 */
const props = withDefaults(defineProps<WorkflowEditorProps>(), { workflowId: null, template: null, lesson: null })
const emit = defineEmits<{ saved: [id: string]; lesson: [id: string] }>()

const { t, locale } = useI18n()
const settings = useSettingsStore()
const { defs, def, query: catalogQuery } = useNodeCatalog()
const actions = useWorkflowActions()

const flowId = `workflow-${Math.random().toString(36).slice(2)}`
const {
  addSelectedNodes,
  fitView,
  getSelectedEdges,
  getSelectedNodes,
  onConnect,
  onConnectEnd,
  onConnectStart,
  project,
  removeSelectedElements,
  setCenter,
  vueFlowRef,
  getViewport,
  setViewport,
} = useVueFlow(flowId)

/* --- hold Space and drag to move the canvas --- */
const spacePan = useSpacePan({ get: getViewport, set: (next) => void setViewport(next) })

// Typed as plain refs: unwrapping Vue Flow's node type is too deep for TypeScript.
const nodes = ref([]) as Ref<EditorNode[]>
const edges = ref([]) as Ref<EditorEdge[]>
const name = ref('')
const description = ref('')
const active = ref(false)
const react = ref(false)
const maxAmount = ref('')
const savedSnapshot = ref('')
const loaded = ref(false)

/* --- simple / advanced mode, remembered per viewer --- */
const SIMPLE_KEY = 'workflow.editor.simple'
function readSimple(): boolean {
  try {
    return localStorage.getItem(SIMPLE_KEY) !== 'false'
  } catch {
    return true
  }
}
const simple = ref(readSimple())
watch(simple, (value) => {
  try {
    localStorage.setItem(SIMPLE_KEY, String(value))
  } catch {
    /* private window: the mode is not remembered */
  }
})

/* --- loading --- */
const lesson = computed(() => WORKFLOW_LESSONS.find((entry) => entry.id === props.lesson) ?? null)
const workflow = useQuery({
  queryKey: computed(() => WORKFLOW_KEYS.one(props.workflowId ?? '')),
  queryFn: () => call<Workflow>('get_workflow', { id: props.workflowId }),
  enabled: computed(() => Boolean(props.workflowId)),
})

function loadGraph(graph: WorkflowGraph) {
  const flow = toFlow(graph, defs.value)
  nodes.value = flow.nodes
  edges.value = flow.edges
}


const graph = computed(() => toGraph(nodes.value, edges.value))
const graphJson = computed(() => JSON.stringify(graph.value))
const settingsJson = computed(() =>
  JSON.stringify([name.value, description.value, active.value, react.value, maxAmount.value]),
)
const dirty = computed(() => loaded.value && savedSnapshot.value !== graphJson.value + settingsJson.value)

/* --- undo / redo --- */
let restoring = false
const history = useHistory((snapshot) => {
  restoring = true
  loadGraph(parseGraph(snapshot))
  void nextTick(() => {
    restoring = false
  })
})
let recordTimer: ReturnType<typeof setTimeout> | undefined
watch(graphJson, (json) => {
  if (!loaded.value || restoring) return
  clearTimeout(recordTimer)
  recordTimer = setTimeout(() => history.record(json), 250)
})

/* Loads after everything it touches is declared: a cached workflow fires at once. */
watch(
  [() => workflow.data.value, () => catalogQuery.data.value, () => props.template],
  ([stored, catalog]) => {
    if (loaded.value || !catalog) return
    if (props.workflowId) {
      if (!stored) return
      name.value = stored.name
      description.value = stored.description
      active.value = stored.active
      react.value = stored.react_to_workflows
      maxAmount.value = stored.max_amount ?? ''
      loadGraph(parseGraph(stored.graph))
    } else if (lesson.value) {
      name.value = t('workflow.lesson.workflowName', { title: t(`workflow.lesson.${lesson.value.id}.title`) })
      loadGraph(lesson.value.start((key) => t(key)))
    } else {
      const template = WORKFLOW_TEMPLATES.find((entry) => entry.id === props.template)
      name.value = template ? t(`workflow.template.${template.id}.name`) : t('workflow.newName')
      loadGraph(template ? template.graph((key) => t(key)) : emptyGraph())
    }
    loaded.value = true
    savedSnapshot.value = props.workflowId ? graphJson.value + settingsJson.value : ''
    history.reset(graphJson.value)
    void nextTick(() => fitView({ padding: 0.2, maxZoom: 1 }))
  },
  { immediate: true },
)

/* --- checks while editing --- */
const issues = ref<GraphIssue[]>([])
let checkTimer: ReturnType<typeof setTimeout> | undefined
watch(
  graphJson,
  (json) => {
    clearTimeout(checkTimer)
    checkTimer = setTimeout(async () => {
      try {
        issues.value = await call<GraphIssue[]>('validate_workflow', { input: { graph: json } })
      } catch {
        // A graph that does not parse is reported by «Сохранить».
      }
    }, 300)
  },
  { immediate: true },
)
const issuesOf = (nodeId: string) => issues.value.filter((issue) => issue.node_id === nodeId)
const errors = computed(() => issues.value.filter((issue) => issue.level === 'error'))

/* --- the last emulation or run, shown on the canvas --- */
const report = ref<RunReport | null>(null)
const stepOf = computed(() => {
  const map = new Map<string, TraceStep>()
  for (const step of report.value?.run.trace ?? []) map.set(step.node_id, step)
  return map
})
watch(report, (value) => {
  const labels = value?.edge_values ?? {}
  edges.value = edges.value.map((edge) => ({ ...edge, label: labels[edge.id] ?? undefined }))
})

function display(id: string, data: EditorNode['data']): WorkflowNodeData {
  const nodeDef = def(data?.kind ?? '')
  return {
    def: nodeDef,
    kind: data?.kind ?? '',
    label: data?.label ?? '',
    disabled: data?.disabled ?? false,
    issues: issuesOf(id),
    summary: nodeSummary(nodeDef, data?.params ?? {}, locale.value),
    step: stepOf.value.get(id) ?? null,
  }
}

/* --- wiring --- */
const kindOf = (id: string) => nodes.value.find((node) => node.id === id)?.data?.kind ?? ''
const outType = (nodeId: string, port: string | null | undefined) => portType(def(kindOf(nodeId)), port ?? '', 'out')
const inType = (nodeId: string, port: string | null | undefined) => portType(def(kindOf(nodeId)), port ?? '', 'in')

function isValidConnection(connection: Connection): boolean {
  if (connection.source === connection.target) return false
  const from = outType(connection.source, connection.sourceHandle)
  const to = inType(connection.target, connection.targetHandle)
  return Boolean(from && to && portAccepts(to, from))
}

/** Join two ports; a data input keeps one wire, so a new one replaces the old. */
function connect(source: string, sourcePort: string, target: string, targetPort: string) {
  const into = inType(target, targetPort)
  if (into !== 'flow') {
    edges.value = edges.value.filter((edge) => !(edge.target === target && edge.targetHandle === targetPort))
  }
  const id = `${source}.${sourcePort}-${target}.${targetPort}`
  if (edges.value.some((edge) => edge.id === id)) return
  edges.value = [
    ...edges.value,
    toFlowEdge({ id, from: source, from_port: sourcePort, to: target, to_port: targetPort }, defs.value, kindOf),
  ]
}

let connected = false
let dragFrom: { nodeId: string; port: string; side: 'out' | 'in' } | null = null
onConnectStart((params) => {
  connected = false
  dragFrom =
    params.nodeId && params.handleId
      ? { nodeId: params.nodeId, port: params.handleId, side: params.handleType === 'target' ? 'in' : 'out' }
      : null
})
onConnect((connection) => {
  connected = true
  connect(connection.source, connection.sourceHandle ?? '', connection.target, connection.targetHandle ?? '')
})
onConnectEnd((event) => {
  // Dropped on empty canvas: offer the nodes that fit this port.
  if (connected || !dragFrom || !event) return
  const point = 'clientX' in event ? event : event.changedTouches[0]
  const type =
    dragFrom.side === 'out' ? outType(dragFrom.nodeId, dragFrom.port) : inType(dragFrom.nodeId, dragFrom.port)
  if (!type) return
  openQuickAdd(point.clientX, point.clientY, { mode: 'port', ...dragFrom, type })
})

/* --- adding nodes --- */
function newNode(kind: string, position: { x: number; y: number }, params: Record<string, unknown> = {}): EditorNode {
  return {
    id: shortId('n'),
    type: 'wf',
    position,
    data: { kind, params: { ...params }, label: '', disabled: false },
  }
}

/** Screen point → canvas point. */
function toCanvas(clientX: number, clientY: number) {
  const bounds = vueFlowRef.value?.getBoundingClientRect()
  return project({ x: clientX - (bounds?.left ?? 0), y: clientY - (bounds?.top ?? 0) })
}

function centerPoint() {
  const bounds = vueFlowRef.value?.getBoundingClientRect()
  return bounds ? { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 3 } : { x: 400, y: 200 }
}

function add(kind: string, at = centerPoint(), params: Record<string, unknown> = {}): EditorNode {
  const node = newNode(kind, toCanvas(at.x, at.y), params)
  nodes.value = [...nodes.value, node]
  return node
}

/** Palette click: next to the selected node and joined to it when it fits. */
function addFromPalette(kind: string) {
  const selected = getSelectedNodes.value[0]
  const nodeDef = def(kind)
  const node = add(kind)
  if (selected && nodeDef) {
    node.position = { x: selected.position.x + 300, y: selected.position.y }
    const from = def(selected.data?.kind ?? '')
    for (const output of from?.outputs ?? []) {
      const input = nodeDef.inputs.find((port) => portAccepts(port.type, output.type))
      if (input && (output.type === 'flow') === (input.type === 'flow')) {
        connect(selected.id, output.name, node.id, input.name)
        break
      }
    }
  }
  select(node.id)
}

function onDrop(event: DragEvent) {
  const kind = event.dataTransfer?.getData('application/x-workflow-node')
  if (!kind) return
  select(add(kind, { x: event.clientX, y: event.clientY }).id)
}

/* --- Ctrl+Space --- */
const quick = ref<{ left: number; top: number; at: { x: number; y: number }; context: QuickAddContext } | null>(null)
const pointer = ref(centerPoint())

function openQuickAdd(clientX: number, clientY: number, context: QuickAddContext) {
  const left = Math.min(clientX, window.innerWidth - 340)
  const top = Math.min(clientY, window.innerHeight - 420)
  quick.value = { left, top, at: { x: clientX, y: clientY }, context }
}

/** Ctrl+Space where the cursor is: on a selected wire, after a selected node, or free. */
function quickAddHere() {
  const at = pointer.value
  const edge = getSelectedEdges.value[0]
  if (edge) {
    const fromType = outType(edge.source, edge.sourceHandle)
    const toType = inType(edge.target, edge.targetHandle)
    if (fromType && toType) {
      openQuickAdd(at.x, at.y, { mode: 'edge', edgeId: edge.id, fromType, toType })
      return
    }
  }
  const selected = getSelectedNodes.value[0]
  const selectedDef = def(selected?.data?.kind ?? '')
  const output = selectedDef?.outputs.find((port) => port.name === 'out') ?? selectedDef?.outputs[0]
  if (selected && output) {
    openQuickAdd(at.x, at.y, { mode: 'port', nodeId: selected.id, port: output.name, type: output.type, side: 'out' })
    return
  }
  openQuickAdd(at.x, at.y, { mode: 'free' })
}

function onQuickPick(choice: QuickAddChoice) {
  const state = quick.value
  quick.value = null
  if (!state) return
  const node = add(choice.kind, state.at, choice.params)
  const context = state.context
  if (context.mode === 'port' && choice.port) {
    if (context.side === 'out') connect(context.nodeId, context.port, node.id, choice.port)
    else connect(node.id, choice.port, context.nodeId, context.port)
  } else if (context.mode === 'edge' && choice.port) {
    const old = edges.value.find((edge) => edge.id === context.edgeId)
    const source = nodes.value.find((entry) => entry.id === old?.source)
    const target = nodes.value.find((entry) => entry.id === old?.target)
    // Inserted into a wire: it sits halfway along it.
    if (source && target) {
      node.position = {
        x: Math.round((source.position.x + target.position.x) / 2),
        y: Math.round((source.position.y + target.position.y) / 2) + 40,
      }
    }
    if (old && choice.port && choice.outPort) {
      edges.value = edges.value.filter((edge) => edge.id !== old.id)
      connect(old.source, old.sourceHandle ?? '', node.id, choice.port)
      connect(node.id, choice.outPort, old.target, old.targetHandle ?? '')
    }
  }
  select(node.id)
}

/* --- selection and inspector --- */
const selectedNode = computed(() => {
  const selected = getSelectedNodes.value
  return selected.length === 1 && !isNote(selected[0]) ? selected[0] : null
})
const selectedDef = computed(() => def(selectedNode.value?.data?.kind ?? ''))
const helpOpen = ref(false)

function select(nodeId: string) {
  removeSelectedElements()
  void nextTick(() => {
    const target = nodes.value.find((entry) => entry.id === nodeId)
    if (target) addSelectedNodes([target as never])
  })
}

function focusNode(nodeId: string) {
  const node = nodes.value.find((entry) => entry.id === nodeId)
  if (!node) return
  select(nodeId)
  void setCenter(node.position.x + 120, node.position.y + 60, { zoom: 1, duration: 300 })
}

function patchNode(nodeId: string, patch: (data: NonNullable<EditorNode['data']>) => NonNullable<EditorNode['data']>) {
  nodes.value = nodes.value.map((node) => (node.id === nodeId && node.data ? { ...node, data: patch(node.data) } : node))
}

const wiredFrom = computed<Record<string, string>>(() => {
  const node = selectedNode.value
  if (!node) return {}
  const out: Record<string, string> = {}
  for (const edge of edges.value.filter((entry) => entry.target === node.id)) {
    const sourceDef = def(kindOf(edge.source))
    const sourceNode = nodes.value.find((entry) => entry.id === edge.source)
    const port = sourceDef?.outputs.find((entry) => entry.name === edge.sourceHandle)
    const title = sourceNode?.data?.label || (sourceDef ? localized(sourceDef.title, locale.value) : edge.source)
    out[edge.targetHandle ?? ''] = port ? `${title} · ${localized(port.label, locale.value)}` : title
  }
  return out
})

function unwire(port: string) {
  const node = selectedNode.value
  if (!node) return
  edges.value = edges.value.filter((edge) => !(edge.target === node.id && edge.targetHandle === port))
}

function removeSelected() {
  const ids = new Set(getSelectedNodes.value.map((node) => node.id))
  const edgeIds = new Set(getSelectedEdges.value.map((edge) => edge.id))
  nodes.value = nodes.value.filter((node) => !ids.has(node.id))
  edges.value = edges.value.filter((edge) => !edgeIds.has(edge.id) && !ids.has(edge.source) && !ids.has(edge.target))
}

function toggleSelected() {
  for (const node of getSelectedNodes.value.filter((entry) => !isNote(entry))) patchNode(node.id, (data) => ({ ...data, disabled: !data.disabled }))
}

/* --- notes --- */
function addNote() {
  const at = centerPoint()
  const note: EditorNode = {
    id: shortId(NOTE_PREFIX),
    type: 'note',
    position: toCanvas(at.x, at.y),
    data: { text: '', color: '' } as unknown as EditorNode['data'],
  }
  nodes.value = [...nodes.value, note]
}

function noteText(id: string, text: string) {
  nodes.value = nodes.value.map((node) =>
    node.id === id ? { ...node, data: { ...(node.data as unknown as EditorNoteData), text } as unknown as EditorNode['data'] } : node,
  )
}

/* --- copy, paste, duplicate --- */
let clipboard: WorkflowGraph | null = null

function copySelected() {
  const ids = new Set(getSelectedNodes.value.map((node) => node.id))
  if (!ids.size) return
  clipboard = toGraph(
    nodes.value.filter((node) => ids.has(node.id)),
    edges.value.filter((edge) => ids.has(edge.source) && ids.has(edge.target)),
  )
}

function paste(offset = 40) {
  if (!clipboard) return
  const fresh = new Map(clipboard.nodes.map((node) => [node.id, shortId('n')]))
  const copies = clipboard.nodes.map((node) => ({
    ...newNode(node.kind, { x: node.position.x + offset, y: node.position.y + offset }, node.params),
    id: fresh.get(node.id) ?? shortId('n'),
    data: { kind: node.kind, params: { ...node.params }, label: node.label, disabled: node.disabled },
  }))
  nodes.value = [...nodes.value, ...copies]
  for (const edge of clipboard.edges) {
    connect(fresh.get(edge.from) ?? '', edge.from_port, fresh.get(edge.to) ?? '', edge.to_port)
  }
  removeSelectedElements()
  void nextTick(() => addSelectedNodes(nodes.value.filter((node) => copies.some((copy) => copy.id === node.id)) as never))
}

function duplicate() {
  copySelected()
  paste()
}

/* --- save, run, emulate --- */
const base = computed(() => settings.baseCurrency)
const draft = computed<WorkflowDraft>(() => ({
  name: name.value.trim(),
  description: description.value,
  active: active.value,
  graph: graphJson.value,
  react_to_workflows: react.value,
  max_amount: maxAmount.value.trim() ? maxAmount.value.trim().replace(',', '.') : null,
}))
/** Save stays enabled; a missing name or errors on a workflow switched on show under the toolbar. */
const form = useFormValidation(() => ({
  name: required(name.value),
  graph: active.value && errors.value.length ? t('workflow.missing.errors') : null,
}))

function save() {
  actions.save.mutate(
    { id: props.workflowId, draft: draft.value },
    {
      onSuccess: (saved) => {
        savedSnapshot.value = graphJson.value + settingsJson.value
        emit('saved', saved.workflow.id)
      },
    },
  )
}

/** A version put back: the canvas shows it, and it counts as saved. */
function onRestored(restored: Workflow) {
  active.value = restored.active
  loadGraph(parseGraph(restored.graph))
  report.value = null
  void nextTick(() => {
    savedSnapshot.value = graphJson.value + settingsJson.value
    history.reset(graphJson.value)
  })
}

/** Keep what the last emulation did as a test of this workflow. */
function saveAsTest() {
  const result = report.value
  if (!props.workflowId || !result) return
  const queued = result.queued.reduce((sum, batch) => sum + batch.transactions.length, 0)
  actions.saveTest.mutate({
    workflow_id: props.workflowId,
    test: {
      id: '',
      name: t('workflow.tests.defaultName', { when: new Date().toLocaleString(locale.value) }),
      trigger_node: result.run.trace[0]?.node_id ?? null,
      at: result.run.started_at,
      expect: {
        outcome: outcomeKey(result.run.outcome),
        transactions: result.transactions.length,
        queued,
        notices: result.notices.length,
      },
    },
  })
}

function emulate() {
  actions.dryRun.mutate(
    { graph: graphJson.value, workflowId: props.workflowId },
    { onSuccess: (result) => (report.value = result) },
  )
}

function runNow() {
  if (!props.workflowId) return
  actions.runNow.mutate({ id: props.workflowId }, { onSuccess: (result) => (report.value = result) })
}

/* --- keyboard --- */
const hotkeysOpen = ref(false)

function onKeydown(event: KeyboardEvent) {
  const ctrl = event.ctrlKey || event.metaKey
  const key = event.key.toLowerCase()
  if (ctrl && key === 's') {
    event.preventDefault()
    form.submit(save, event)
    return
  }
  if (ctrl && key === 'enter') {
    event.preventDefault()
    emulate()
    return
  }
  if (event.key === 'F1') {
    event.preventDefault()
    helpOpen.value = !helpOpen.value
    return
  }
  if (ctrl && event.code === 'Space') {
    event.preventDefault()
    quickAddHere()
    return
  }
  if (isTyping(event) || quick.value) return
  if (ctrl && key === 'z') {
    event.preventDefault()
    if (event.shiftKey) history.redo()
    else history.undo()
  } else if (ctrl && key === 'y') {
    event.preventDefault()
    history.redo()
  } else if (ctrl && key === 'd') {
    event.preventDefault()
    duplicate()
  } else if (ctrl && key === 'c') {
    copySelected()
  } else if (ctrl && key === 'v') {
    paste()
  } else if (ctrl && key === 'e') {
    event.preventDefault()
    toggleSelected()
  } else if (ctrl && key === 'a') {
    event.preventDefault()
    addSelectedNodes(nodes.value as never)
  } else if (!ctrl && key === 'f') {
    void fitView({ padding: 0.2, duration: 300 })
  } else if (event.key === 'Delete' || event.key === 'Backspace') {
    event.preventDefault()
    removeSelected()
  }
}

function onPointerMove(event: PointerEvent) {
  pointer.value = { x: event.clientX, y: event.clientY }
}

function beforeUnload(event: BeforeUnloadEvent) {
  if (dirty.value && !lesson.value) event.preventDefault()
}
onMounted(() => window.addEventListener('beforeunload', beforeUnload))
onBeforeUnmount(() => {
  window.removeEventListener('beforeunload', beforeUnload)
  clearTimeout(recordTimer)
  clearTimeout(checkTimer)
})

/** A lesson is practice: leaving it never asks to save. */
defineExpose({ dirty: computed(() => dirty.value && !lesson.value) })
</script>

<template>
  <div
    class="flex h-full min-h-0 flex-col gap-3"
    role="application"
    :aria-label="t('workflow.editorLabel')"
    tabindex="-1"
    @keydown="onKeydown"
  >
    <!-- Toolbar: name, on/off, save, emulate, run. -->
    <div class="flex flex-wrap items-center gap-2" data-tour="workflow-toolbar">
      <div class="w-64" :data-invalid="form.invalidAttr('name')">
        <Input
          v-model="name"
          :aria-label="t('workflow.name')"
          :aria-invalid="Boolean(form.error('name'))"
          :placeholder="t('workflow.name')"
          :class="form.error('name') ? 'border-destructive' : ''"
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
      <Button variant="ghost" size="icon" :aria-label="t('workflow.hotkeys')" :aria-pressed="hotkeysOpen" @click="hotkeysOpen = !hotkeysOpen">
        <Keyboard class="size-4" />
      </Button>
      <Button variant="ghost" size="icon" :aria-label="t('workflow.note.add')" :title="t('workflow.note.add')" @click="addNote">
        <StickyNote class="size-4" />
      </Button>
      <Button variant="outline" data-tour="workflow-quick-button" @click="quickAddHere">
        <Plus class="size-4" />
        {{ t('workflow.addNode') }}
      </Button>
      <Button variant="outline" data-tour="workflow-emulate" :disabled="actions.dryRun.isPending.value || !nodes.length" @click="emulate">
        <FlaskConical class="size-4" />
        {{ t('workflow.emulate') }}
      </Button>
      <Button
        v-if="props.workflowId"
        variant="outline"
        :disabled="actions.runNow.isPending.value || dirty"
        :title="dirty ? t('workflow.saveFirst') : undefined"
        @click="runNow"
      >
        <Play class="size-4" />
        {{ t('workflow.runNow') }}
      </Button>
      <Button data-tour="workflow-save" :disabled="actions.save.isPending.value" @click="form.submit(save, $event)">
        <Save class="size-4" />
        {{ t('actions.save') }}
      </Button>
    </div>
    <p v-if="form.firstMessage()" class="text-xs text-destructive" role="alert">{{ form.firstMessage() }}</p>

    <div v-if="hotkeysOpen" class="rounded-lg border border-border bg-card p-3"><HotkeysHelp /></div>

    <div class="grid min-h-0 flex-1 grid-cols-[14rem_minmax(0,1fr)_20rem] gap-3">
      <NodePalette :simple="simple" class="min-h-0" @add="addFromPalette" />

      <div class="flex min-h-0 flex-col gap-3">
        <section
          class="wf-canvas relative min-h-[28rem] flex-1 overflow-hidden rounded-lg border border-border bg-background"
          data-tour="workflow-canvas"
          :aria-label="t('workflow.canvasLabel')"
          @drop.prevent="onDrop"
          @dragover.prevent
          :data-space-pan="spacePan.held.value ? (spacePan.panning.value ? 'grabbing' : 'grab') : undefined"
          @pointerdown.capture="spacePan.onPress"
          @mousedown.capture="spacePan.onPress"
          @pointermove="onPointerMove"
        >
          <VueFlow
            :id="flowId"
            v-model:nodes="nodes"
            v-model:edges="edges"
            :is-valid-connection="isValidConnection"
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
                @text="noteText"
              />
            </template>
            <Background :gap="20" />
            <Controls :show-interactive="false" />
            <MiniMap pannable zoomable />
          </VueFlow>
          <div
            v-if="loaded && !nodes.length"
            class="pointer-events-none absolute inset-0 flex items-center justify-center p-8 text-center text-sm text-muted-foreground"
          >
            {{ t('workflow.canvas.empty') }}
          </div>
        </section>
        <RunPanel
          v-if="report"
          :report="report"
          :can-save-test="Boolean(props.workflowId) && report.run.dry_run && !dirty"
          @focus="focusNode"
          @close="report = null"
          @save-test="saveAsTest"
        />
      </div>

      <div class="flex min-h-0 flex-col gap-3 overflow-y-auto">
        <LessonPanel
          v-if="lesson"
          :lesson="lesson"
          :graph="graph"
          :report="report"
          :checking="actions.dryRun.isPending.value"
          @check="emulate"
          @next="(id) => emit('lesson', id)"
        />
        <div class="rounded-lg border border-border bg-card p-3">
        <NodeInspector
          v-if="selectedNode"
          v-model:help="helpOpen"
          :node="selectedNode as EditorNode"
          :def="selectedDef"
          :wired-from="wiredFrom"
          :issues="issuesOf(selectedNode.id)"
          @param="(param, value) => selectedNode && patchNode(selectedNode.id, (data) => ({ ...data, params: { ...data.params, [param]: value } }))"
          @label="(value) => selectedNode && patchNode(selectedNode.id, (data) => ({ ...data, label: value }))"
          @toggle="toggleSelected"
          @unwire="unwire"
          @remove="removeSelected"
        />
        <WorkflowSettingsPanel
          v-else
          v-model:description="description"
          v-model:max-amount="maxAmount"
          v-model:react="react"
          :issues="issues"
          :base="base"
          @focus="focusNode"
        />
        <WorkflowHistoryPanel
          v-if="!selectedNode && workflow.data.value"
          class="mt-4 border-t border-border pt-4"
          :workflow="workflow.data.value"
          :dirty="dirty"
          @restored="onRestored"
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
      @pick="onQuickPick"
      @close="quick = null"
    />
    <button v-if="quick" type="button" class="fixed inset-0 z-40 cursor-default" :aria-label="t('actions.close')" @click="quick = null" />
  </div>
</template>
