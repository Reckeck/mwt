/**
 * Browser-mock half of node workflows for `pnpm dev`. The catalog is the real
 * one (written by `mise run gen-types`); a run only walks the flow wires and
 * says which nodes it passed — the executor itself lives in Rust.
 */

import { catalogMap, emptyGraph, parseGraph, portAccepts, portType } from '@/shared/lib/workflow'

import catalogJson from './ipc-mock-workflow-catalog.json'

const catalog = catalogJson as unknown as NodeDef[]
const defs = catalogMap(catalog)

const workflows: Workflow[] = []
const runs: WorkflowRun[] = []
const queue: QueuedActions[] = []
const vars: WorkflowVar[] = []

const uuid = () => crypto.randomUUID()
const now = () => new Date().toISOString().replace(/\.\d{3}Z$/, 'Z')

function issue(code: string, ru: string, en: string, nodeId: string | null = null, edgeId: string | null = null) {
  return { level: 'error', code, node_id: nodeId, edge_id: edgeId, text_ru: ru, text_en: en } as GraphIssue
}

/** The checks that matter while clicking around; the full set is in Rust. */
function validate(graph: WorkflowGraph): GraphIssue[] {
  const out: GraphIssue[] = []
  for (const node of graph.nodes) {
    if (!defs.has(node.kind)) out.push(issue('unknown_node', 'Неизвестный узел', 'Unknown node', node.id))
  }
  const hasTrigger = graph.nodes.some((node) => !node.disabled && defs.get(node.kind)?.group === 'trigger')
  if (graph.nodes.length && !hasTrigger) {
    out.push(
      issue(
        'no_trigger',
        'Нет триггера: добавьте узел, с которого схема начинается (например, «По расписанию»)',
        'No trigger: add a node the workflow starts from (for instance «On a schedule»)',
      ),
    )
  }
  for (const edge of graph.edges) {
    const fromNode = graph.nodes.find((node) => node.id === edge.from)
    const toNode = graph.nodes.find((node) => node.id === edge.to)
    const from = portType(defs.get(fromNode?.kind ?? ''), edge.from_port, 'out')
    const to = portType(defs.get(toNode?.kind ?? ''), edge.to_port, 'in')
    if (!from || !to || !portAccepts(to, from)) {
      out.push(issue('type_mismatch', 'Связь не подходит по типу', 'The wire does not fit', edge.to, edge.id))
    }
  }
  return out
}

/** Walk the flow from a trigger, as a stand-in for a real run. */
function fakeRun(graph: WorkflowGraph, workflowId: string, triggerNode: string | null, dryRun: boolean): RunReport {
  const triggers = graph.nodes.filter((node) => defs.get(node.kind)?.group === 'trigger')
  const start =
    triggers.find((node) => node.id === triggerNode) ??
    triggers.find((node) => node.kind === 'trigger.manual') ??
    triggers[0]
  const trace: TraceStep[] = []
  const seen = new Set<string>()
  const visit = (id: string) => {
    if (seen.has(id) || seen.size > 100) return
    seen.add(id)
    const node = graph.nodes.find((entry) => entry.id === id)
    const def = defs.get(node?.kind ?? '')
    if (!node || !def) return
    trace.push({
      node_id: id,
      kind: node.kind,
      status: node.disabled ? 'skipped' : 'ok',
      text_ru: `«${def.title.ru}» (браузерный мок: только порядок узлов)`,
      text_en: `«${def.title.en}» (browser mock: node order only)`,
      values: {},
    })
    for (const edge of graph.edges.filter((entry) => entry.from === id)) {
      if (portType(def, edge.from_port, 'out') === 'flow') visit(edge.to)
    }
  }
  if (start) visit(start.id)
  const run: WorkflowRun = {
    id: uuid(),
    workflow_id: workflowId,
    trigger: start ? start.kind.replace('trigger.', '') : '',
    started_at: now(),
    outcome: start ? 'done' : { failed: { message: 'the workflow has no trigger' } },
    created: [],
    queued: 0,
    trace,
    dry_run: dryRun,
  }
  return { run, transactions: [], queued: [], notices: [], events: [], edge_values: {} }
}

function find(id: string): Workflow {
  const workflow = workflows.find((entry) => entry.id === id)
  if (!workflow) throw { code: 'MISSING', message: `workflow ${id}` }
  return workflow
}

function save(id: string | null, draft: WorkflowDraft): SavedWorkflow {
  if (!draft.name.trim()) throw { code: 'INVALID_INPUT', message: 'name the workflow' }
  const issues = validate(parseGraph(draft.graph))
  if (draft.active && issues.some((entry) => entry.level === 'error')) {
    throw { code: 'INVALID_INPUT', message: `fix the workflow's errors before switching it on: ${issues[0].text_en}` }
  }
  const stamp = now()
  if (!id) {
    const workflow: Workflow = {
      id: uuid(),
      name: draft.name.trim(),
      description: draft.description,
      active: draft.active,
      graph: draft.graph,
      created_at: stamp,
      updated_at: stamp,
      react_to_workflows: draft.react_to_workflows,
      max_amount: draft.max_amount,
      versions: [],
    }
    workflows.push(workflow)
    return { workflow, issues }
  }
  const workflow = find(id)
  if (workflow.graph !== draft.graph) {
    workflow.versions = [...workflow.versions, { saved_at: workflow.updated_at, graph: workflow.graph }].slice(-20)
  }
  Object.assign(workflow, { ...draft, name: draft.name.trim(), updated_at: stamp })
  return { workflow, issues }
}

const sameScope = (left: VarScope, right: VarScope) => JSON.stringify(left) === JSON.stringify(right)

/** Answer a workflow command, or `undefined` when `cmd` is not one. */
export function mockWorkflows(cmd: string, args: Record<string, unknown> | undefined): unknown {
  const input = args?.input as Record<string, unknown> | undefined
  switch (cmd) {
    case 'workflow_node_catalog':
      return catalog
    case 'validate_workflow':
      return validate(parseGraph(String(input?.graph ?? '')))
    case 'list_workflows':
      return workflows
    case 'get_workflow':
      return find(String(args?.id))
    case 'save_workflow':
      return save((input?.id as string | null) || null, input?.workflow as WorkflowDraft)
    case 'set_workflow_active': {
      const workflow = find(String(input?.id))
      if (input?.active && validate(parseGraph(workflow.graph)).length) {
        throw { code: 'INVALID_INPUT', message: "fix the workflow's errors before switching it on" }
      }
      workflow.active = Boolean(input?.active)
      return workflow
    }
    case 'delete_workflow': {
      const index = workflows.findIndex((entry) => entry.id === args?.id)
      if (index >= 0) workflows.splice(index, 1)
      return null
    }
    case 'restore_workflow_version': {
      const workflow = find(String(input?.id))
      const version = workflow.versions.find((entry) => entry.saved_at === input?.saved_at)
      if (!version) throw { code: 'MISSING', message: 'version' }
      return save(workflow.id, { ...workflow, graph: version.graph, active: false })
    }
    case 'dry_run_workflow':
      return fakeRun(
        parseGraph(String(input?.graph ?? '')),
        String(input?.workflow_id ?? 'draft'),
        (input?.trigger_node as string) ?? null,
        true,
      )
    case 'run_workflow_now': {
      const workflow = find(String(input?.id))
      const report = fakeRun(parseGraph(workflow.graph), workflow.id, (input?.trigger_node as string) ?? null, false)
      runs.unshift(report.run)
      return report
    }
    case 'list_workflow_runs':
      return runs.filter((run) => !input?.workflow_id || run.workflow_id === input.workflow_id)
    case 'list_workflow_queue':
      return queue
    case 'confirm_workflow_queued':
    case 'discard_workflow_queued': {
      const index = queue.findIndex((entry) => entry.id === args?.id)
      if (index >= 0) queue.splice(index, 1)
      return cmd === 'confirm_workflow_queued' ? [] : null
    }
    case 'list_workflow_vars':
      return vars
    case 'set_workflow_var': {
      const scope = input?.scope as VarScope
      const name = String(input?.name ?? '').trim()
      const existing = vars.find((entry) => sameScope(entry.scope, scope) && entry.name === name)
      if (existing) Object.assign(existing, { value: String(input?.value), updated_at: now() })
      else vars.push({ scope, name, value: String(input?.value), updated_at: now() })
      return null
    }
    case 'delete_workflow_var': {
      const index = vars.findIndex(
        (entry) => sameScope(entry.scope, input?.scope as VarScope) && entry.name === input?.name,
      )
      if (index >= 0) vars.splice(index, 1)
      return null
    }
    case 'export_workflow': {
      const workflow = find(String(args?.id))
      return JSON.stringify(
        {
          format: 'trade-app-workflow',
          version: 1,
          name: workflow.name,
          description: workflow.description,
          graph: JSON.parse(workflow.graph || JSON.stringify(emptyGraph())),
        },
        null,
        2,
      )
    }
    case 'import_workflow': {
      const file = JSON.parse(String(args?.json ?? '{}')) as Record<string, unknown>
      if (file.format !== 'trade-app-workflow') throw { code: 'INVALID_INPUT', message: 'not a workflow file' }
      return save(null, {
        name: String(file.name ?? ''),
        description: String(file.description ?? ''),
        active: false,
        graph: JSON.stringify(file.graph ?? emptyGraph()),
        react_to_workflows: false,
        max_amount: null,
      })
    }
    default:
      return undefined
  }
}
