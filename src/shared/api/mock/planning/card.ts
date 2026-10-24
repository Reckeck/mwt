/** The automation card and the journal (Auto-1 · 3–4), as `get_automation_card` and `list_automation_journal` give them. */

import { txId, txLegs, txMeta } from '@/shared/lib/transactions'
import { mockNotices, mockTransactions } from '../demo'
import { runs as workflowRuns, waits, workflows } from '../workflows/state'
import { nextScheme } from './mine'
import { automations, fail, iso, now, plannedDates, runs } from './state'

const LOOKAHEAD_DAYS = 400
const RUNS_ON_CARD = 5

const noMoney = (): CardMoney => ({ amount: null, currency: null, from_store: null, to_store: null })

/** Where the money leaves, where it comes, how much: by what arrives, else by what leaves. */
function moneyOf(tx: Transaction): CardMoney {
  const legs = txLegs(tx).filter((leg) => leg.role === 'principal')
  const out = legs.find((leg) => Number(leg.amount) < 0)
  const into = legs.find((leg) => Number(leg.amount) > 0)
  const leg = into ?? out
  return {
    amount: leg ? String(Math.abs(Number(leg.amount))) : null,
    currency: leg ? leg.asset.symbol.toLowerCase() : null,
    from_store: out?.store_id ?? null,
    to_store: into?.store_id ?? null,
  }
}

const moneyOfTx = (id: string | undefined) => {
  const tx = id ? mockTransactions.find((entry) => txId(entry) === id) : undefined
  return tx ? moneyOf(tx) : noMoney()
}

function storeOf(value: unknown): string | null {
  if (typeof value === 'string') return value || null
  return (value as { store?: string } | null)?.store ?? null
}

const text = (params: Record<string, unknown>, name: string) => {
  const raw = params[name]
  return raw === undefined || raw === null || String(raw).trim() === '' ? null : String(raw).trim()
}

function source(graph: WorkflowGraph, node: string, port: string) {
  const edge = graph.edges.find((entry) => entry.to === node && entry.to_port === port)
  const from = edge && graph.nodes.find((entry) => entry.id === edge.from)
  return from && edge ? { node: from, port: edge.from_port } : null
}

/** The share a «Математика» node set to percent takes, and that node. */
function percentOf(graph: WorkflowGraph, node: WorkflowNode) {
  const math = source(graph, node.id, 'amount')?.node
  if (math?.kind !== 'logic.math' || text(math.params, 'op') !== 'percent') return null
  const share = text(math.params, 'b')
  return share ? { share, math } : null
}

function actionOf(graph: WorkflowGraph, node: WorkflowNode): CardAction {
  const params = node.params
  const action: CardAction = { kind: node.kind, percent: null, money: noMoney(), title: '' }
  if (node.kind === 'action.transaction') {
    const account = source(graph, node.id, 'account')?.node
    action.kind = text(params, 'tx_kind') ?? 'transfer'
    action.percent = percentOf(graph, node)?.share ?? null
    action.money = {
      amount: text(params, 'amount'),
      currency: text(params, 'asset')?.toLowerCase() ?? null,
      to_store: storeOf(params.to),
      from_store:
        storeOf(params.account) ?? (account?.kind === 'trigger.transaction' ? storeOf(account.params.store) : null),
    }
  } else if (node.kind === 'action.distribute') {
    action.kind = 'distribute'
    action.money.from_store = storeOf(params.from)
  } else if (node.kind === 'action.notify' || node.kind === 'action.reminder') {
    action.kind = node.kind.slice('action.'.length)
    action.title = text(params, 'title') ?? ''
  }
  return action
}

/** What a run waiting for «да» is about to do: the saved values, the share taken of the income. */
function askedMoney(graph: WorkflowGraph, memo: string): CardMoney {
  const saved = JSON.parse(memo || '[]') as [string, string, WfValue][]
  const value = (node: string, port: string) => saved.find(([from, out]) => from === node && out === port)?.[2]
  const node = graph.nodes.find((entry) => !entry.disabled && entry.kind === 'action.transaction')
  if (!node) return noMoney()
  const money = actionOf(graph, node).money
  const percent = percentOf(graph, node)
  const income = percent && source(graph, percent.math.id, 'a')
  const got = income && value(income.node.id, income.port)
  if (percent && got?.type === 'money') {
    money.amount = String((Number(got.value.amount) * Number(percent.share)) / 100)
    money.currency = got.value.currency
  }
  return money
}

function schemeEntry(workflow: Workflow, run: WorkflowRun): JournalEntry {
  const outcome = run.outcome
  const [status, note]: [JournalStatus, string] =
    outcome === 'done'
      ? ['done', '']
      : 'asking' in outcome
        ? ['asking', outcome.asking.question]
        : 'waiting' in outcome
          ? ['waiting', outcome.waiting.until]
          : 'partial' in outcome
            ? ['partial', outcome.partial.failed.join('; ')]
            : 'failed' in outcome
              ? ['failed', outcome.failed.message]
              : 'stopped' in outcome
                ? ['stopped', outcome.stopped.reason]
                : 'rejected' in outcome
                  ? ['rejected', '']
                  : ['skipped', outcome.dropped.reason]
  const wait = waits.find((entry) => entry.run_id === run.id)
  const money = run.created[0]
    ? moneyOfTx(run.created[0])
    : wait
      ? askedMoney(JSON.parse(workflow.graph) as WorkflowGraph, wait.memo)
      : noMoney()
  const kind: MineKind = workflow.automation ? 'repeat' : 'scheme'
  return {
    id: run.id,
    kind,
    source_id: workflow.id,
    name: workflow.name,
    at: run.started_at,
    status,
    money,
    wait_id: status === 'asking' ? (wait?.id ?? null) : null,
    note,
  }
}

function repeatEntry(automation: Automation, run: AutomationRun): JournalEntry {
  const outcome = run.outcome
  const created = typeof outcome === 'object' && 'created' in outcome ? outcome.created.tx_id : undefined
  const status: JournalStatus = created ? 'done' : outcome === 'skipped' ? 'skipped' : 'rejected'
  return {
    id: run.id,
    kind: 'repeat',
    source_id: automation.id,
    name: automation.name,
    at: run.processed_at,
    status,
    money: moneyOfTx(created),
    wait_id: null,
    note: '',
  }
}

export function automationJournal(limit = 100): JournalEntry[] {
  const out: JournalEntry[] = []
  for (const run of workflowRuns.filter((entry) => !entry.dry_run)) {
    const workflow = workflows.find((entry) => entry.id === run.workflow_id)
    if (workflow) out.push(schemeEntry(workflow, run))
  }
  for (const run of runs) {
    const automation = automations.find((entry) => entry.id === run.automation_id)
    if (automation) out.push(repeatEntry(automation, run))
  }
  for (const notice of mockNotices) {
    if (notice.category !== 'reminder' && notice.category !== 'price_alert') continue
    out.push({
      id: notice.key,
      kind: notice.category === 'reminder' ? 'reminder' : 'alert',
      source_id: notice.key.split(':')[1] ?? '',
      name: notice.title,
      at: notice.delivered_at || notice.at,
      status: 'shown',
      money: noMoney(),
      wait_id: null,
      note: notice.body,
    })
  }
  return out.sort((left, right) => right.at.localeCompare(left.at)).slice(0, Math.min(limit, 500))
}

const nextOf = (automation: Automation) => {
  const until = new Date(now().getTime() + LOOKAHEAD_DAYS * 86_400_000)
  const start = new Date(automation.starts_at)
  return plannedDates(automation.schedule, start, until, automation.workday_shift).find((date) => date > now())
}

function repeatCard(automation: Automation): AutomationCard {
  const [kind] = Object.keys(automation.template)
  const next = nextOf(automation)
  return {
    kind: 'repeat',
    id: automation.id,
    name: automation.name,
    description: '',
    active: automation.active,
    schedule: automation.schedule,
    workday: automation.workday_shift,
    follows: null,
    trigger: { kind: 'schedule' },
    actions: [
      { kind: kind ?? 'payment', percent: null, money: moneyOf(automation.template as Transaction), title: '' },
    ],
    asks: false,
    next_at: next ? iso(next) : null,
    runs: [],
  }
}

/** A scheme on an income of a category runs when the soonest repeat booking that category does. */
function followIncome(card: AutomationCard, category: string) {
  const soonest = automations
    .filter(
      (automation) => automation.active && txMeta(automation.template as Transaction).categories.includes(category),
    )
    .map((automation) => ({ automation, next: nextOf(automation) }))
    .filter((entry): entry is { automation: Automation; next: Date } => !!entry.next)
    .sort((left, right) => left.next.getTime() - right.next.getTime())[0]
  if (!soonest) return
  Object.assign(card, {
    schedule: soonest.automation.schedule,
    workday: soonest.automation.workday_shift,
    follows: soonest.automation.name,
    next_at: iso(soonest.next),
  })
}

function schemeCard(workflow: Workflow): AutomationCard {
  const graph = JSON.parse(workflow.graph) as WorkflowGraph
  const live = graph.nodes.filter((node) => !node.disabled)
  const trigger = live.find((node) => node.kind.startsWith('trigger.'))
  const card: AutomationCard = {
    kind: 'scheme',
    id: workflow.id,
    name: workflow.name,
    description: workflow.description,
    active: workflow.active,
    schedule: null,
    workday: 'none',
    follows: null,
    trigger: { kind: 'manual' },
    actions: live
      .filter((node) => node.kind.startsWith('action.') && node.kind !== 'action.confirm')
      .map((node) => actionOf(graph, node)),
    asks: live.some((node) => node.kind === 'action.confirm'),
    next_at: nextScheme(graph),
    runs: [],
  }
  if (trigger?.kind === 'trigger.schedule') card.trigger = { kind: 'schedule' }
  else if (trigger?.kind === 'trigger.transaction') {
    const category = text(trigger.params, 'category')
    card.trigger = {
      kind: 'transaction',
      tx_kind: text(trigger.params, 'tx_kind') ?? 'any',
      category,
      store: storeOf(trigger.params.store),
    }
    if (category) followIncome(card, category)
  } else if (trigger && trigger.kind !== 'trigger.manual') card.trigger = { kind: 'other', node_kind: trigger.kind }
  return card
}

export function automationCard(id: string): AutomationCard {
  const automation = automations.find((entry) => entry.id === id)
  const workflow = workflows.find((entry) => entry.id === id && !entry.automation)
  const card = automation
    ? repeatCard(automation)
    : workflow
      ? schemeCard(workflow)
      : fail(`automation ${id}`, 'MISSING')
  card.runs = automationJournal(500)
    .filter((entry) => entry.source_id === id)
    .slice(0, RUNS_ON_CARD)
  return card
}
