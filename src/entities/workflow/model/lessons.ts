/**
 * Lessons for the node editor: a task on the canvas, a few checks that tick
 * as the user builds, and an emulation on a copy of the data at the end —
 * nothing a lesson does touches the real accounts. Texts:
 * `workflow.lesson.<id>.{title,goal,hint}` and `workflow.lesson.<id>.check.<key>`.
 */

export interface LessonCheck {
  key: string
  /** Ticks from the graph alone, or needs the last emulation. */
  test: (graph: WorkflowGraph, report: RunReport | null) => boolean
}

export interface Lesson {
  id: string
  /** The canvas the lesson starts from; texts in the user's language. */
  start: (t: (key: string) => string) => WorkflowGraph
  checks: LessonCheck[]
}

type Params = Record<string, unknown>

function node(id: string, kind: string, column: number, row: number, params: Params = {}): WorkflowNode {
  return { id, kind, params, position: { x: column * 300, y: row * 170 }, disabled: false, label: '' }
}

function wire(from: string, fromPort: string, to: string, toPort: string): WorkflowEdge {
  return { id: `${from}.${fromPort}-${to}.${toPort}`, from, from_port: fromPort, to, to_port: toPort }
}

function graph(nodes: WorkflowNode[], edges: WorkflowEdge[] = []): WorkflowGraph {
  return { version: 1, nodes, edges, comments: [] }
}

const kindsOf = (value: WorkflowGraph, kind: string) => value.nodes.filter((entry) => entry.kind === kind)
const has = (value: WorkflowGraph, kind: string) => kindsOf(value, kind).length > 0

/** A wire from a node of `fromKind` (port) to a node of `toKind` (port). */
function wired(value: WorkflowGraph, fromKind: string, fromPort: string, toKind: string, toPort: string): boolean {
  const kind = (id: string) => value.nodes.find((entry) => entry.id === id)?.kind
  return value.edges.some(
    (edge) =>
      kind(edge.from) === fromKind &&
      edge.from_port === fromPort &&
      kind(edge.to) === toKind &&
      edge.to_port === toPort,
  )
}

/** The emulation went through and passed a node of this kind. */
function ran(report: RunReport | null, kind?: string): boolean {
  if (report?.run.outcome !== 'done') return false
  return !kind || report.run.trace.some((step) => step.kind === kind && step.status !== 'error')
}

export const WORKFLOW_LESSONS: Lesson[] = [
  {
    id: 'first',
    start: () => graph([]),
    checks: [
      { key: 'trigger', test: (value) => has(value, 'trigger.manual') },
      { key: 'notify', test: (value) => has(value, 'action.notify') },
      { key: 'wire', test: (value) => wired(value, 'trigger.manual', 'out', 'action.notify', 'in') },
      {
        key: 'text',
        test: (value) =>
          kindsOf(value, 'action.notify').some((entry) => String(entry.params.text ?? '').trim()) ||
          value.edges.some((edge) => edge.to_port === 'text'),
      },
      { key: 'emulate', test: (_value, report) => ran(report, 'action.notify') },
    ],
  },
  {
    id: 'data',
    start: (t) =>
      graph(
        [
          node('start', 'trigger.manual', 0, 0),
          node('tell', 'action.notify', 2, 0, { text: t('workflow.lesson.data.text') }),
        ],
        [wire('start', 'out', 'tell', 'in')],
      ),
    checks: [
      { key: 'now', test: (value) => has(value, 'data.now') },
      { key: 'build', test: (value) => wired(value, 'data.now', 'now', 'logic.text', 'a') },
      { key: 'into', test: (value) => wired(value, 'logic.text', 'text', 'action.notify', 'text') },
      { key: 'emulate', test: (_value, report) => ran(report, 'action.notify') },
    ],
  },
  {
    id: 'condition',
    start: (t) =>
      graph(
        [
          node('daily', 'trigger.schedule', 0, 0, { every: 'day', time: '10:00', interval: '1' }),
          node('tell', 'action.notify', 3, 0, { text: t('workflow.lesson.condition.text') }),
        ],
        [],
      ),
    checks: [
      { key: 'workday', test: (value) => has(value, 'data.workday') },
      {
        key: 'branch',
        test: (value) =>
          wired(value, 'data.workday', 'workday', 'logic.branch', 'condition') ||
          wired(value, 'data.workday', 'weekend', 'logic.branch', 'condition'),
      },
      { key: 'flow', test: (value) => wired(value, 'trigger.schedule', 'out', 'logic.branch', 'in') },
      {
        key: 'notify',
        test: (value) =>
          wired(value, 'logic.branch', 'true', 'action.notify', 'in') ||
          wired(value, 'logic.branch', 'false', 'action.notify', 'in'),
      },
      { key: 'emulate', test: (_value, report) => ran(report, 'logic.branch') },
    ],
  },
  {
    id: 'quick',
    start: (t) =>
      graph(
        [
          node('start', 'trigger.manual', 0, 0),
          node('salary', 'data.constant', 0, 1, { value_type: 'money', number: '50000', currency: 'rub' }),
          node('text', 'logic.text', 2, 1, { template: t('workflow.lesson.quick.text') }),
          node('tell', 'action.notify', 2, 0),
        ],
        [
          wire('start', 'out', 'tell', 'in'),
          wire('salary', 'value', 'text', 'a'),
          wire('text', 'text', 'tell', 'text'),
        ],
      ),
    checks: [
      {
        key: 'percent',
        test: (value) =>
          kindsOf(value, 'logic.math').some(
            (entry) => entry.params.op === 'percent' && String(entry.params.b) === '10',
          ),
      },
      { key: 'between', test: (value) => wired(value, 'data.constant', 'value', 'logic.math', 'a') },
      { key: 'onward', test: (value) => wired(value, 'logic.math', 'result', 'logic.text', 'a') },
      {
        key: 'emulate',
        test: (_value, report) => ran(report) && (report?.notices ?? []).some((notice) => notice.body.includes('5000')),
      },
    ],
  },
  {
    id: 'money',
    start: () => graph([node('start', 'trigger.manual', 0, 0)]),
    checks: [
      { key: 'confirm', test: (value) => wired(value, 'trigger.manual', 'out', 'action.confirm', 'in') },
      { key: 'operation', test: (value) => wired(value, 'action.confirm', 'out', 'action.transaction', 'in') },
      {
        key: 'accounts',
        test: (value) =>
          kindsOf(value, 'action.transaction').some((entry) => entry.params.account && entry.params.amount),
      },
      { key: 'queued', test: (_value, report) => ran(report) && (report?.queued.length ?? 0) > 0 },
    ],
  },
]

/** Lessons finished on this device (a per-viewer convenience). */
const DONE_KEY = 'workflow.lessons.done'

export function lessonsDone(): string[] {
  try {
    return JSON.parse(localStorage.getItem(DONE_KEY) ?? '[]') as string[]
  } catch {
    return []
  }
}

export function markLessonDone(id: string) {
  try {
    const done = new Set(lessonsDone())
    done.add(id)
    localStorage.setItem(DONE_KEY, JSON.stringify([...done]))
  } catch {
    /* private window: progress is not remembered */
  }
}
