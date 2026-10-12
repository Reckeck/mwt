/**
 * Ready-made workflows to start from. Accounts, limits and goals are left
 * empty: the editor marks what to fill in, the emulation shows the result.
 */

export interface WorkflowTemplate {
  id: string
  /** `workflow.template.<id>.name` / `.desc` in the locales. */
  /** Texts inside the nodes come in the user's language. */
  graph: (t: (key: string) => string) => WorkflowGraph
}

type Params = Record<string, unknown>

function node(id: string, kind: string, column: number, row: number, params: Params = {}): WorkflowNode {
  return { id, kind, params, position: { x: column * 300, y: row * 170 }, disabled: false, label: '' }
}

function wire(from: string, fromPort: string, to: string, toPort: string): WorkflowEdge {
  return { id: `${from}.${fromPort}-${to}.${toPort}`, from, from_port: fromPort, to, to_port: toPort }
}

function graph(nodes: WorkflowNode[], edges: WorkflowEdge[]): WorkflowGraph {
  return { version: 1, nodes, edges, comments: [] }
}

export const WORKFLOW_TEMPLATES: WorkflowTemplate[] = [
  {
    id: 'salaryShare',
    graph: (_t) =>
      graph(
        [
          node('salary', 'trigger.transaction', 0, 0, { tx_kind: 'topup', category: 'salary' }),
          node('share', 'logic.math', 1, 1, { op: 'percent', b: '10', round: '2' }),
          node('save', 'action.distribute', 2, 0, { mode: 'percent', targets: [] }),
        ],
        [
          wire('salary', 'out', 'save', 'in'),
          wire('salary', 'amount', 'share', 'a'),
          wire('salary', 'account', 'save', 'from'),
          wire('share', 'result', 'save', 'amount'),
        ],
      ),
  },
  {
    id: 'lowBalance',
    graph: (t) =>
      graph(
        [
          node('low', 'trigger.balance', 0, 0, { condition: 'below', value: '5000' }),
          node('once', 'logic.throttle', 1, 0, { hours: '24' }),
          node('text', 'logic.text', 1, 1, { template: t('workflow.template.lowBalance.text') }),
          node('tell', 'action.notify', 2, 0, { title: '' }),
        ],
        [
          wire('low', 'out', 'once', 'in'),
          wire('once', 'out', 'tell', 'in'),
          wire('low', 'balance', 'text', 'a'),
          wire('text', 'text', 'tell', 'text'),
        ],
      ),
  },
  {
    id: 'weeklyBuy',
    graph: (_t) =>
      graph(
        [
          node('friday', 'trigger.schedule', 0, 0, { every: 'week', weekday: 4, time: '10:00', interval: '1' }),
          node('check', 'action.confirm', 1, 0),
          node('buy', 'action.transaction', 2, 0, { tx_kind: 'convert', amount: '50', to_amount: '0' }),
          node('count', 'var.add', 3, 0, { name: 'buys', delta: '1', scope: 'workflow' }),
        ],
        [wire('friday', 'out', 'check', 'in'), wire('check', 'out', 'buy', 'in'), wire('buy', 'out', 'count', 'in')],
      ),
  },
  {
    id: 'workdayPayment',
    graph: (t) =>
      graph(
        [
          node('tenth', 'trigger.schedule', 0, 0, { every: 'month', day: 10, time: '09:00', interval: '1' }),
          node('day', 'data.workday', 0, 1, { calendar: 'ru', weekend: 'sat_sun' }),
          node('branch', 'logic.branch', 1, 0),
          node('pay', 'action.notify', 2, 0, { text: t('workflow.template.workdayPayment.today'), title: '' }),
          node('moved', 'logic.text', 1, 2, { template: t('workflow.template.workdayPayment.moved') }),
          node('later', 'action.notify', 2, 1, { title: '' }),
        ],
        [
          wire('tenth', 'out', 'branch', 'in'),
          wire('day', 'workday', 'branch', 'condition'),
          wire('branch', 'true', 'pay', 'in'),
          wire('branch', 'false', 'later', 'in'),
          wire('day', 'next_workday', 'moved', 'a'),
          wire('moved', 'text', 'later', 'text'),
        ],
      ),
  },
  {
    id: 'budgetWatch',
    graph: (t) =>
      graph(
        [
          node('evening', 'trigger.schedule', 0, 0, { every: 'day', time: '20:00', interval: '1' }),
          node('budget', 'data.budget', 0, 1),
          node('over', 'logic.compare', 1, 1, { op: 'gte', b: '90' }),
          node('guard', 'logic.guard', 1, 0, { reason: t('workflow.template.budgetWatch.fine') }),
          node('text', 'logic.text', 2, 1, { template: t('workflow.template.budgetWatch.text') }),
          node('tell', 'action.notify', 2, 0, { title: '' }),
        ],
        [
          wire('evening', 'out', 'guard', 'in'),
          wire('budget', 'used', 'over', 'a'),
          wire('over', 'result', 'guard', 'condition'),
          wire('guard', 'out', 'tell', 'in'),
          wire('budget', 'used', 'text', 'a'),
          wire('text', 'text', 'tell', 'text'),
        ],
      ),
  },
]
