/** «10 % зарплаты» of the demo vault (Auto-1 · 3–5): the share of each salary, asked «да» before it goes to cash. */

import { txId, txMeta } from '@/shared/lib/transactions'
import { runs, waits, workflows } from '../workflows/state'
import { iso, now, uuid } from './state'

const SALARY = '1575'

/** What `pay_yourself_first` builds: the income, its share, the question, the transfer — in two rows, as on Auto-2 · 13. */
function shareGraph(from: string, to: string): WorkflowGraph {
  const node = (
    id: string,
    kind: string,
    params: Record<string, unknown>,
    left: number,
    top: number,
  ): WorkflowNode => ({
    id,
    kind,
    params,
    position: { x: left, y: top },
    disabled: false,
    label: '',
  })
  const edge = (from_node: string, from_port: string, to_node: string, to_port: string): WorkflowEdge => ({
    id: `${from_node}-${to_node}-${to_port}`,
    from: from_node,
    from_port,
    to: to_node,
    to_port,
  })
  return {
    version: 3,
    nodes: [
      node('income', 'trigger.transaction', { tx_kind: 'topup', category: 'salary', store: { store: from } }, 0, 0),
      node('share', 'logic.math', { op: 'percent', b: '10' }, 240, 170),
      node('ask', 'action.confirm', { question: 'Отложить 10 % зарплаты на «Наличные»?', timeout_hours: 72 }, 480, 0),
      node('save', 'action.transaction', { tx_kind: 'transfer', to: { store: to } }, 480, 170),
    ],
    edges: [
      edge('income', 'amount', 'share', 'a'),
      edge('income', 'out', 'ask', 'in'),
      edge('income', 'account', 'save', 'account'),
      edge('ask', 'true', 'save', 'in'),
      edge('share', 'result', 'save', 'amount'),
    ],
    comments: [],
  }
}

function run(workflowId: string, startedAt: string, outcome: RunOutcome, created: string[]): WorkflowRun {
  return {
    id: uuid(),
    workflow_id: workflowId,
    trigger: 'income',
    started_at: startedAt,
    outcome,
    created,
    queued: 0,
    trace: [],
    dry_run: false,
  }
}

/** The scheme, a run per booked share in `deposits`, and the latest salary still waiting for «да». */
export function seedShare(from: string, to: string, deposits: Transaction[]) {
  const created = iso(now())
  const graph = shareGraph(from, to)
  const workflow: Workflow = {
    id: uuid(),
    name: '10 % зарплаты',
    description: '',
    active: true,
    graph: JSON.stringify(graph),
    created_at: created,
    updated_at: created,
    react_to_workflows: false,
    max_amount: null,
    max_amount_currency: null,
    versions: [],
    tests: [],
  }
  workflows.push(workflow)
  for (const tx of deposits) runs.push(run(workflow.id, txMeta(tx).timestamp, 'done', [txId(tx)]))
  const asked = new Date(now().getTime() - 86_400_000)
  asked.setHours(10, 5, 0, 0)
  const question = String(graph.nodes[2].params.question)
  const asking = run(workflow.id, iso(asked), { asking: { node_id: 'ask', question, until: null } }, [])
  runs.unshift(asking)
  const income: WfValue = { type: 'money', value: { amount: SALARY, currency: 'usd' } }
  waits.push({
    id: uuid(),
    workflow_id: workflow.id,
    run_id: asking.id,
    node_id: 'ask',
    resume_at: iso(new Date(asked.getTime() + 72 * 3_600_000)),
    memo: JSON.stringify([
      ['income', 'amount', income],
      ['income', 'account', { type: 'account', value: { store: from } }],
    ]),
    queue_mode: false,
    trigger: 'income',
    kind: 'confirm',
    question,
    created_at: iso(asked),
  })
}
