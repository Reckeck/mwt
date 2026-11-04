/** Gallery templates for `pnpm dev`: the real graphs are built in core, here a trigger and a notice stand in. */

import { save } from './state'

const field = (key: string, kind: TemplateFieldKind, fallback = ''): TemplateField => ({
  key,
  kind,
  required: true,
  default: fallback,
})

export const templates: WorkflowTemplateInfo[] = [
  {
    id: 'pay_yourself_first',
    moves_money: true,
    fields: [
      field('category', 'category', 'salary'),
      field('pct', 'number', '10'),
      field('target', 'store'),
      { ...field('source', 'store'), required: false },
    ],
  },
  { id: 'rates_watch', moves_money: false, fields: [field('currency', 'currency', 'rub')] },
  { id: 'budget_pace', moves_money: false, fields: [field('budget', 'budget'), field('margin', 'number', '10')] },
  { id: 'big_spend', moves_money: false, fields: [field('min_amount', 'number', '10000')] },
  { id: 'subscription_watch', moves_money: false, fields: [field('category', 'category', 'subscriptions')] },
  {
    id: 'round_up',
    moves_money: true,
    fields: [
      field('from', 'store'),
      field('to', 'store'),
      field('step', 'number', '100'),
      field('weekday', 'weekday', '6'),
    ],
  },
  { id: 'topup_by_shares', moves_money: false, fields: [field('day', 'day', '1')] },
]

const node = (id: string, kind: string, column: number, params: Record<string, unknown>) => ({
  id,
  kind,
  params,
  position: { x: column * 300, y: 0 },
  disabled: false,
  label: '',
})

export function fromTemplate(input: WorkflowFromTemplateInput): SavedWorkflow {
  const template = templates.find((entry) => entry.id === input.template_id)
  if (!template) throw { code: 'NOT_FOUND', message: `template ${input.template_id}` }
  const missing = template.fields.find((entry) => entry.required && !input.params[entry.key]?.trim())
  if (missing) throw { code: 'INVALID_INPUT', message: `${missing.key}: required` }
  const graph = {
    version: 3,
    nodes: [
      node('when', 'trigger.schedule', 0, { every: 'day', interval: '1', time: '10:00' }),
      node('tell', 'action.notify', 1, { title: template.id, text: template.id }),
    ],
    edges: [{ id: 'when.out-tell.in', from: 'when', from_port: 'out', to: 'tell', to_port: 'in' }],
    comments: [],
  }
  return save(null, {
    name: template.id,
    description: '',
    active: true,
    graph: JSON.stringify(graph),
    react_to_workflows: false,
    max_amount: null,
    max_amount_currency: null,
  })
}
