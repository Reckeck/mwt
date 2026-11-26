import type { TourStep } from './tours.types.ts'

/** Every «Operations» view starts with the strip over its four routes, then its own content. */
function operations(own: TourStep[]): TourStep[] {
  return [{ target: 'section-tabs', key: 'transactions.tabs', side: 'bottom' }, ...own]
}

/** Every automations tab starts with the tabs, then its own content. */
function workflows(own: TourStep[]): TourStep[] {
  return [{ target: 'section-tabs', key: 'workflows.tabs', side: 'bottom' }, ...own]
}

/** The tabs «More» holds share the planning strip text. */
function planning(own: TourStep[]): TourStep[] {
  return [{ target: 'section-tabs', key: 'planning.tabs', side: 'bottom' }, ...own]
}

/** The node editor: new and saved workflows alike (a saved one shows this tour, see `TOUR_ALIASES`). */
const WORKFLOW_EDITOR: TourStep[] = [
  { target: 'workflow-canvas', key: 'workflowEditor.canvas', side: 'top' },
  { target: 'workflow-words', key: 'workflowEditor.words', side: 'bottom' },
  { target: 'workflow-steps', key: 'workflowEditor.steps', side: 'top' },
  // On an empty editor and on a touch screen.
  { target: 'workflow-add-first', key: 'workflowEditor.addFirst', side: 'top' },
  { target: 'workflow-add-touch', key: 'workflowEditor.addTouch', side: 'left' },
  { target: 'workflow-quick-hint', key: 'workflowEditor.quick', side: 'top' },
  { target: 'workflow-settings', key: 'workflowEditor.inspector', side: 'left' },
  { target: 'workflow-inspector', key: 'workflowEditor.node', side: 'left' },
  { target: 'workflow-node-flows', key: 'workflowEditor.flows', side: 'left' },
  { target: 'workflow-lesson', key: 'workflowEditor.lesson', side: 'left' },
  { target: 'workflow-tests', key: 'workflowEditor.tests', side: 'left' },
  { target: 'workflow-emulate', key: 'workflowEditor.emulate', side: 'bottom' },
  { target: 'workflow-run', key: 'workflowEditor.runFrom', side: 'bottom' },
  { target: 'workflow-emulation-question', key: 'workflowEditor.question', side: 'top' },
  { target: 'workflow-event-runs', key: 'workflowEditor.eventRuns', side: 'top' },
  { target: 'workflow-active', key: 'workflowEditor.active', side: 'bottom' },
  { target: 'workflow-edit-commands', key: 'workflowEditor.editCommands', side: 'bottom' },
  { target: 'workflow-save', key: 'workflowEditor.save', side: 'bottom' },
  { target: 'workflow-discard', key: 'workflowEditor.discard', side: 'bottom' },
  { target: 'workflow-draft', key: 'workflowEditor.draft', side: 'bottom' },
  { target: 'workflow-automation', key: 'workflowEditor.automation', side: 'bottom' },
]

/** Tours of the «Operations» section and of automations; steps whose element is not on the screen are skipped. */
export const OPERATIONS_TOURS: Record<string, TourStep[]> = {
  transactions: operations([
    { target: 'tx-filter', key: 'transactions.filter', side: 'bottom' },
    { target: 'section-content', key: 'transactions.list', side: 'top' },
  ]),
  'transactions-calendar': operations([
    { target: 'section-content', key: 'transactionsCalendar.content', side: 'top' },
    { target: 'calendar-balance', key: 'transactionsCalendar.balance', side: 'bottom' },
  ]),
  'transactions-bonuses': operations([{ target: 'bonuses-cashback', key: 'bonuses.cashback', side: 'top' }]),
  'crypto-operations': operations([{ target: 'crypto-operations', key: 'cryptoOperations.list', side: 'top' }]),
  automations: workflows([
    { target: 'automations-create', key: 'automationsMine.create', side: 'bottom' },
    { target: 'automations-import', key: 'automationsMine.import', side: 'bottom' },
  ]),
  'automations-journal': workflows([
    { target: 'automations-journal', key: 'workflows.runs', side: 'top' },
    { target: 'journal-confirm', key: 'workflows.questions', side: 'left' },
    { target: 'workflow-queue', key: 'workflows.queue', side: 'top' },
  ]),
  'automations-workflows': workflows([
    { target: 'workflow-lessons', key: 'workflows.lessons', side: 'bottom' },
    { target: 'workflow-new', key: 'workflows.new', side: 'left' },
    { target: 'workflow-vars-open', key: 'workflows.vars', side: 'left' },
  ]),
  'automations-reminders': planning([{ target: 'section-content', key: 'planningReminders.content', side: 'top' }]),
  'automations-alerts': planning([{ target: 'section-content', key: 'planningAlerts.content', side: 'top' }]),
  'automations-sources': planning([
    { target: 'section-content', key: 'planningSources.content', side: 'top' },
    { target: 'source-url', key: 'planningSources.url', side: 'bottom' },
    { target: 'source-picker', key: 'planningSources.picker', side: 'top' },
    { target: 'source-method', key: 'planningSources.method', side: 'top' },
    { target: 'source-test', key: 'planningSources.test', side: 'top' },
    { target: 'source-uses', key: 'planningSources.uses', side: 'top' },
  ]),
  'workflow-new': WORKFLOW_EDITOR,
}
