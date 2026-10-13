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
const waits: WorkflowWait[] = []

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

/** Flow outputs the mock never takes: it has no data to decide them. */
const MOCK_SKIPPED_PORTS = new Set(['error', 'skipped', 'false', 'case2', 'case3', 'case4', 'other'])

/**
 * Walk the flow from a trigger (or on from a question), as a stand-in for a
 * real run. A branch goes its «Да» way; «Ждать подтверждения» goes the way
 * answered, or stops the run and asks.
 */
function fakeRun(
  graph: WorkflowGraph,
  workflowId: string,
  triggerNode: string | null,
  dryRun: boolean,
  answers: Record<string, boolean> = {},
  resume: { nodeId: string; port: string } | null = null,
): RunReport {
  const triggers = graph.nodes.filter((node) => defs.get(node.kind)?.group === 'trigger')
  const start =
    triggers.find((node) => node.id === triggerNode) ??
    triggers.find((node) => node.kind === 'trigger.manual') ??
    triggers[0]
  const trace: TraceStep[] = []
  const seen = new Set<string>()
  let asking: { node_id: string; question: string; until: string | null } | null = null
  const follow = (id: string, port: string) => {
    for (const edge of graph.edges.filter((entry) => entry.from === id && entry.from_port === port)) {
      if (!asking) visit(edge.to)
    }
  }
  const visit = (id: string) => {
    if (seen.has(id) || seen.size > 100 || asking) return
    seen.add(id)
    const node = graph.nodes.find((entry) => entry.id === id)
    const def = defs.get(node?.kind ?? '')
    if (!node || !def) return
    const step = (status: TraceStep['status'], ru: string, en: string) =>
      trace.push({ node_id: id, kind: node.kind, status, text_ru: ru, text_en: en, values: {} })
    if (node.kind === 'action.confirm' && !node.disabled) {
      const question = String(node.params.question ?? '').trim() || 'Продолжить?'
      const answer = answers[id]
      if (answer === undefined) {
        asking = { node_id: id, question, until: null }
        step(
          'waiting',
          `«${def.title.ru}»: жду вашего ответа — ${question}`,
          `«${def.title.en}»: waiting for your answer: ${question}`,
        )
        return
      }
      step(
        'ok',
        `«${def.title.ru}»: ответ «${answer ? 'Да' : 'Нет'}»`,
        `«${def.title.en}»: answered «${answer ? 'Yes' : 'No'}»`,
      )
      follow(id, answer ? 'true' : 'false')
      return
    }
    step(
      node.disabled ? 'skipped' : 'ok',
      `«${def.title.ru}» (браузерный мок: только порядок узлов)`,
      `«${def.title.en}» (browser mock: node order only)`,
    )
    for (const output of def.outputs) {
      if (output.type === 'flow' && !MOCK_SKIPPED_PORTS.has(output.name)) follow(id, output.name)
    }
  }
  if (resume) follow(resume.nodeId, resume.port)
  else if (start) visit(start.id)
  const outcome: RunOutcome = asking
    ? { asking }
    : start || resume
      ? 'done'
      : { failed: { message: 'the workflow has no trigger' } }
  const run: WorkflowRun = {
    id: uuid(),
    workflow_id: workflowId,
    trigger: start ? start.kind.replace('trigger.', '') : '',
    started_at: now(),
    outcome,
    created: [],
    queued: 0,
    trace,
    dry_run: dryRun,
  }
  return { run, transactions: [], queued: [], notices: [], events: [], reminders: [], edge_values: {} }
}

/** A live mock run that asked keeps its question for «Очередь». */
function keepQuestion(report: RunReport) {
  const outcome = report.run.outcome
  if (outcome === 'done' || !('asking' in outcome)) return
  waits.push({
    id: uuid(),
    workflow_id: report.run.workflow_id,
    run_id: report.run.id,
    node_id: outcome.asking.node_id,
    resume_at: '',
    memo: '[]',
    queue_mode: false,
    trigger: report.run.trigger,
    kind: 'confirm',
    question: outcome.asking.question,
    created_at: now(),
  })
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
      tests: [],
    }
    workflows.push(workflow)
    return { workflow, issues }
  }
  const workflow = find(id)
  if (workflow.graph !== draft.graph) {
    workflow.versions = [...workflow.versions, { saved_at: workflow.updated_at, graph: workflow.graph }].slice(-20)
  }
  Object.assign(workflow, { ...draft, name: draft.name.trim(), updated_at: stamp, draft: undefined })
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
        (input?.answers as Record<string, boolean> | undefined) ?? {},
      )
    case 'run_workflow_now': {
      const workflow = find(String(input?.id))
      const report = fakeRun(parseGraph(workflow.graph), workflow.id, (input?.trigger_node as string) ?? null, false)
      runs.unshift(report.run)
      keepQuestion(report)
      return report
    }
    case 'save_workflow_draft': {
      const workflow = find(String(input?.id))
      workflow.draft = { saved_at: now(), graph: String(input?.graph ?? '') }
      return workflow
    }
    case 'discard_workflow_draft': {
      const workflow = find(String(args?.id))
      workflow.draft = undefined
      return workflow
    }
    case 'list_workflow_waits':
      return waits
    case 'answer_workflow_wait': {
      const index = waits.findIndex((entry) => entry.id === input?.id && entry.kind === 'confirm')
      if (index < 0) throw { code: 'MISSING', message: `question ${String(input?.id)}` }
      const [wait] = waits.splice(index, 1)
      const workflow = find(wait.workflow_id)
      const port = input?.yes ? 'true' : 'false'
      const report = fakeRun(parseGraph(workflow.graph), workflow.id, null, false, {}, { nodeId: wait.node_id, port })
      runs.unshift(report.run)
      keepQuestion(report)
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
    case 'save_workflow_test': {
      const workflow = find(String(input?.workflow_id))
      const test = { ...(input?.test as WorkflowTest) }
      if (!test.name?.trim()) throw { code: 'INVALID_INPUT', message: 'name the test' }
      if (!test.id) test.id = uuid()
      workflow.tests = [...workflow.tests.filter((entry) => entry.id !== test.id), test]
      return workflow
    }
    case 'delete_workflow_test': {
      const workflow = find(String(input?.workflow_id))
      workflow.tests = workflow.tests.filter((entry) => entry.id !== input?.test_id)
      return workflow
    }
    case 'run_workflow_tests': {
      const workflow = find(String(args?.id))
      return workflow.tests.map((test): WorkflowTestResult => {
        const report = fakeRun(parseGraph(workflow.graph), workflow.id, test.trigger_node, true)
        const got = report.run.outcome === 'done' ? 'done' : 'failed'
        const passed = got === test.expect.outcome
        return {
          test_id: test.id,
          name: test.name,
          passed,
          problems_ru: passed ? [] : [`итог «${got}» вместо «${test.expect.outcome}»`],
          problems_en: passed ? [] : [`outcome «${got}» instead of «${test.expect.outcome}»`],
          report,
        }
      })
    }
    default:
      return undefined
  }
}
