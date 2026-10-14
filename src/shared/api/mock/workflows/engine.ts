/** Checking a graph and walking its flow wires for a fake run. */

import { portAccepts, portType } from '@/shared/lib/workflow'
import { type OrderEdge, actionSources, runsAfter } from '@/shared/lib/workflow-order'
import { defs, issue, now, uuid, waits } from './state'

const orderEdges = (graph: WorkflowGraph): OrderEdge[] =>
  graph.edges.map((edge) => ({ from: edge.from, to: edge.to, toPort: edge.to_port }))
const defOfIn = (graph: WorkflowGraph) => (id: string) =>
  defs.get(graph.nodes.find((node) => node.id === id)?.kind ?? '')

/** A flow wire into a node that already waits on its source by data: it can go. */
function redundantFlows(graph: WorkflowGraph): GraphIssue[] {
  const edges = orderEdges(graph)
  const defOf = defOfIn(graph)
  return graph.nodes.flatMap((node) => {
    const flowIn = graph.edges.filter(
      (edge) =>
        edge.to === node.id &&
        defOf(node.id)?.inputs.some((port) => port.name === edge.to_port && port.type === 'flow'),
    )
    const only = flowIn.length === 1 ? flowIn[0] : undefined
    if (only?.from_port !== 'out' || !actionSources(edges, defOf, node.id).includes(only.from)) return []
    return [
      issue(
        'redundant_flow',
        'Поток повторяет связь по данным: его можно убрать, порядок останется',
        'The flow repeats the data wire: it can go, the order stays',
        null,
        only.id,
      ),
    ]
  })
}

/** The checks that matter while clicking around; the full set is in Rust. */
export function validate(graph: WorkflowGraph): GraphIssue[] {
  const out: GraphIssue[] = [...redundantFlows(graph)]
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
export function fakeRun(
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
  const edges = orderEdges(graph)
  const defOf = defOfIn(graph)
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
      const question = String(node.params.question ?? '').trim()
      const answer = answers[id]
      if (answer === undefined) {
        asking = { node_id: id, question, until: null }
        step(
          'waiting',
          `«${def.title.ru}»: жду вашего ответа — ${question || 'Продолжить?'}`,
          `«${def.title.en}»: waiting for your answer: ${question || 'Go on?'}`,
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
    if (node.disabled) return
    // Data order: nodes with no flow wire in run once all the actions they read have.
    for (const waiting of graph.nodes) {
      const sources = runsAfter(edges, defOf, waiting.id)
      if (!waiting.disabled && sources.includes(id) && sources.every((source) => seen.has(source))) visit(waiting.id)
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
  return { run, transactions: [], queued: [], notices: [], events: [], reminders: [], event_runs: [], edge_values: {} }
}

/** A live mock run that asked keeps its question for «Очередь». */
export function keepQuestion(report: RunReport) {
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
