/** One stop of a tour: the element (by `data-tour`) and the i18n keys of its popover. */
export interface TourStep {
  target: string
  key: string
  side?: 'top' | 'right' | 'bottom' | 'left'
}

/** The tabs, filter and content every analytics report shares; the last step says what the report is. */
function analytics(report: string): TourStep[] {
  return [
    { target: 'section-tabs', key: 'analytics.tabs', side: 'bottom' },
    { target: 'report-filter', key: 'analytics.filter', side: 'bottom' },
    { target: 'section-content', key: `analytics.${report}`, side: 'top' },
    { target: 'report-export', key: 'analytics.export', side: 'left' },
  ]
}

/** Every planning tab starts with the tabs, then its own content. */
function planning(own: TourStep[]): TourStep[] {
  return [{ target: 'section-tabs', key: 'planning.tabs', side: 'bottom' }, ...own]
}

/** Every workflows tab starts with the tabs, then its own content. */
function workflows(own: TourStep[]): TourStep[] {
  return [{ target: 'section-tabs', key: 'workflows.tabs', side: 'bottom' }, ...own]
}

/** The node editor: new and saved workflows alike. */
const WORKFLOW_EDITOR: TourStep[] = [
  { target: 'workflow-canvas', key: 'workflowEditor.canvas', side: 'top' },
  { target: 'workflow-quick-hint', key: 'workflowEditor.quick', side: 'top' },
  { target: 'workflow-settings', key: 'workflowEditor.inspector', side: 'left' },
  { target: 'workflow-node-flows', key: 'workflowEditor.flows', side: 'left' },
  { target: 'workflow-lesson', key: 'workflowEditor.lesson', side: 'left' },
  { target: 'workflow-tests', key: 'workflowEditor.tests', side: 'left' },
  { target: 'workflow-emulate', key: 'workflowEditor.emulate', side: 'bottom' },
  { target: 'workflow-run', key: 'workflowEditor.runFrom', side: 'bottom' },
  { target: 'workflow-emulation-question', key: 'workflowEditor.question', side: 'top' },
  { target: 'workflow-event-runs', key: 'workflowEditor.eventRuns', side: 'top' },
  { target: 'workflow-active', key: 'workflowEditor.active', side: 'bottom' },
  { target: 'workflow-save', key: 'workflowEditor.save', side: 'bottom' },
  { target: 'workflow-discard', key: 'workflowEditor.discard', side: 'bottom' },
  { target: 'workflow-draft', key: 'workflowEditor.draft', side: 'bottom' },
]

/**
 * Tours by route name. Steps whose element is not on the screen are skipped,
 * so one tour serves wallets of every kind. Every route has one — the
 * `check-onboarding` gate says so (see the `onboarding` skill).
 */
export const TOURS: Record<string, TourStep[]> = {
  workflows: workflows([
    { target: 'workflow-lessons', key: 'workflows.lessons', side: 'bottom' },
    { target: 'workflow-templates', key: 'workflows.templates', side: 'bottom' },
    { target: 'workflow-new', key: 'workflows.new', side: 'left' },
    { target: 'workflow-card', key: 'workflows.card', side: 'top' },
  ]),
  'workflows-vars': workflows([{ target: 'workflow-vars', key: 'workflows.vars', side: 'top' }]),
  'workflows-queue': workflows([
    { target: 'workflow-queue', key: 'workflows.queue', side: 'top' },
    { target: 'workflow-questions', key: 'workflows.questions', side: 'top' },
  ]),
  'workflows-runs': workflows([{ target: 'workflow-runs', key: 'workflows.runs', side: 'top' }]),
  'workflow-new': WORKFLOW_EDITOR,
  dashboard: [
    { target: 'dashboard-grid', key: 'dashboard.grid', side: 'top' },
    { target: 'dashboard-add', key: 'dashboard.add', side: 'left' },
    { target: 'sidebar-add-wallet', key: 'dashboard.wallets', side: 'right' },
    { target: 'command-palette', key: 'dashboard.palette', side: 'bottom' },
    { target: 'notification-bell', key: 'dashboard.bell', side: 'bottom' },
    { target: 'sync-indicator', key: 'dashboard.sync', side: 'bottom' },
    { target: 'open-settings', key: 'dashboard.settings', side: 'right' },
    { target: 'tour-help', key: 'dashboard.contextMenu', side: 'bottom' },
  ],
  stores: [
    { target: 'stores-list', key: 'stores.list', side: 'top' },
    { target: 'stores-create', key: 'stores.create', side: 'left' },
  ],
  wallet: [
    { target: 'wallet-operation', key: 'wallet.operation', side: 'bottom' },
    { target: 'wallet-adjust', key: 'wallet.adjust', side: 'bottom' },
    { target: 'wallet-settings', key: 'wallet.settings', side: 'bottom' },
    { target: 'utility-reminders', key: 'wallet.utilityReminders', side: 'bottom' },
    { target: 'chart-currency', key: 'wallet.chartCurrency', side: 'left' },
    { target: 'exchange-views', key: 'wallet.views', side: 'bottom' },
    { target: 'exchange-terminal', key: 'wallet.terminal', side: 'top' },
  ],
  'wallet-settings': [
    { target: 'wallet-purpose', key: 'walletSettings.purpose', side: 'top' },
    { target: 'wallet-envelopes', key: 'walletSettings.envelopes', side: 'top' },
    { target: 'automation-presets', key: 'walletSettings.presets', side: 'bottom' },
    { target: 'exchange-configure-import', key: 'walletSettings.importSetup', side: 'top' },
    { target: 'import-routes', key: 'walletSettings.importRoutes', side: 'top' },
    { target: 'import-kinds', key: 'walletSettings.importKinds', side: 'top' },
    { target: 'import-preview', key: 'walletSettings.importPreview', side: 'top' },
    { target: 'exchange-sync-history', key: 'walletSettings.syncHistory', side: 'top' },
    { target: 'exchange-imported', key: 'walletSettings.imported', side: 'top' },
    { target: 'store-change', key: 'walletSettings.change', side: 'top' },
  ],
  transactions: [
    { target: 'section-tabs', key: 'transactions.tabs', side: 'bottom' },
    { target: 'tx-filter', key: 'transactions.filter', side: 'bottom' },
    { target: 'section-content', key: 'transactions.list', side: 'top' },
  ],
  'transactions-graph': [
    { target: 'section-tabs', key: 'transactions.tabs', side: 'bottom' },
    { target: 'section-content', key: 'transactionsGraph.content', side: 'top' },
  ],
  'transactions-calendar': [
    { target: 'section-tabs', key: 'transactions.tabs', side: 'bottom' },
    { target: 'section-content', key: 'transactionsCalendar.content', side: 'top' },
  ],
  'analytics-cashflow': analytics('cashflow'),
  'analytics-year': analytics('year'),
  'analytics-categories': analytics('categories'),
  'analytics-networth': analytics('networth'),
  'analytics-stores': analytics('stores'),
  'analytics-pnl': analytics('pnl'),
  'analytics-losses': analytics('losses'),
  'analytics-portfolio': analytics('portfolio'),
  'analytics-inflation': analytics('inflation'),
  'directory-contacts': [
    { target: 'contact-quick-create', key: 'directory.create', side: 'right' },
    { target: 'contact-groups', key: 'directory.groups', side: 'right' },
    { target: 'contact-editor', key: 'directory.editor', side: 'left' },
    { target: 'contact-debts', key: 'directory.debts', side: 'top' },
  ],
  'planning-automations': planning([
    { target: 'section-content', key: 'planningAutomations.content', side: 'top' },
    { target: 'automation-simulate', key: 'planningAutomations.simulate', side: 'bottom' },
  ]),
  'planning-budgets': planning([{ target: 'section-content', key: 'planningBudgets.content', side: 'top' }]),
  'planning-goals': planning([{ target: 'section-content', key: 'planningGoals.content', side: 'top' }]),
  'planning-reminders': planning([
    { target: 'section-content', key: 'planningReminders.content', side: 'top' },
    { target: 'reminder-filter', key: 'planningReminders.filter', side: 'bottom' },
  ]),
  'planning-alerts': planning([{ target: 'section-content', key: 'planningAlerts.content', side: 'top' }]),
}

/** Routes with no tour, and why (the `check-onboarding` gate reads this). */
export const TOURLESS_ROUTES: Record<string, string> = {
  'workflow-edit': 'The same editor as workflow-new; it shows that tour (TOUR_ALIASES).',
  unlock: 'Before a vault is open: the unlock screen explains itself.',
  settings: 'A redirect to the settings window, which has its own sections.',
}

/** Routes that show another route's tour: a saved workflow opens the same editor as a new one. */
export const TOUR_ALIASES: Record<string, string> = {
  'workflow-edit': 'workflow-new',
}

/** The tour a route shows. */
export function tourIdOf(routeName: string): string {
  return TOUR_ALIASES[routeName] ?? routeName
}
