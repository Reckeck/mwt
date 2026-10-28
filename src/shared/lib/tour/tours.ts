/** One stop of a tour: the element (by `data-tour`) and the i18n keys of its popover. */
export interface TourStep {
  target: string
  key: string
  side?: 'top' | 'right' | 'bottom' | 'left'
}

/**
 * Every analytics section: its tabs and filter, its own steps, the facts in
 * words and the export. A step whose element is not on screen is skipped.
 */
function analytics(own: TourStep[]): TourStep[] {
  return [
    { target: 'section-tabs', key: 'analytics.tabs', side: 'bottom' },
    { target: 'report-filter', key: 'analytics.filter', side: 'bottom' },
    ...own,
    { target: 'report-insights', key: 'analytics.insights', side: 'bottom' },
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
  { target: 'workflow-words', key: 'workflowEditor.words', side: 'bottom' },
  { target: 'workflow-steps', key: 'workflowEditor.steps', side: 'top' },
  // Only on an empty canvas; skipped once there is a node.
  { target: 'workflow-add-first', key: 'workflowEditor.addFirst', side: 'top' },
  // A phone: the «+» on the canvas instead of Ctrl+Space.
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

/** An empty new workflow: what to do first, not all fourteen stops of the editor. */
export const WORKFLOW_EDITOR_EMPTY: TourStep[] = [
  { target: 'workflow-add-first', key: 'workflowEditor.addFirst', side: 'top' },
  { target: 'workflow-settings', key: 'workflowEditor.inspector', side: 'left' },
  { target: 'workflow-save', key: 'workflowEditor.save', side: 'bottom' },
]

/**
 * Tours by route name. Steps whose element is not on the screen are skipped,
 * so one tour serves wallets of every kind. Every route has one — the
 * `check-onboarding` gate says so (see the `onboarding` skill).
 */
export const TOURS: Record<string, TourStep[]> = {
  'automations-workflows': workflows([
    { target: 'workflow-lessons', key: 'workflows.lessons', side: 'bottom' },
    { target: 'workflow-templates', key: 'workflows.templates', side: 'bottom' },
    { target: 'workflow-new', key: 'workflows.new', side: 'left' },
    { target: 'workflow-card', key: 'workflows.card', side: 'top' },
    { target: 'workflow-vars-open', key: 'workflows.vars', side: 'left' },
  ]),
  'automations-journal': workflows([
    { target: 'automations-journal', key: 'workflows.runs', side: 'top' },
    { target: 'journal-confirm', key: 'workflows.questions', side: 'left' },
    { target: 'workflow-queue', key: 'workflows.queue', side: 'top' },
  ]),
  'workflow-new': WORKFLOW_EDITOR,
  dashboard: [
    { target: 'home-figures', key: 'dashboard.figures', side: 'bottom' },
    { target: 'home-tasks', key: 'dashboard.tasks', side: 'bottom' },
    { target: 'home-changes', key: 'dashboard.changes', side: 'top' },
    { target: 'dashboard-add', key: 'dashboard.add', side: 'top' },
    { target: 'dashboard-grid', key: 'dashboard.grid', side: 'top' },
    { target: 'dash-net-worth', key: 'dashboard.netWorthTile', side: 'bottom' },
    { target: 'dash-month-pace', key: 'dashboard.monthPace', side: 'top' },
    { target: 'dash-upcoming-coverage', key: 'dashboard.coverage', side: 'top' },
    // Only on a phone.
    { target: 'bottom-nav', key: 'dashboard.bottomNav', side: 'top' },
    { target: 'quick-add', key: 'dashboard.quickAdd', side: 'bottom' },
    { target: 'dash-health', key: 'dashboard.healthWidget', side: 'top' },
    { target: 'sidebar-toggle', key: 'dashboard.sidebarToggle', side: 'right' },
    { target: 'sidebar-pinned', key: 'dashboard.pinned', side: 'right' },
    { target: 'command-palette', key: 'dashboard.palette', side: 'bottom' },
    { target: 'notification-bell', key: 'dashboard.bell', side: 'bottom' },
    { target: 'sync-indicator', key: 'dashboard.sync', side: 'bottom' },
    { target: 'open-settings', key: 'dashboard.settings', side: 'right' },
    { target: 'tour-help', key: 'dashboard.contextMenu', side: 'bottom' },
  ],
  // The settings window opens over any screen; a step of a tab that is not open is skipped.
  settings: [
    { target: 'settings-groups', key: 'settings.groups', side: 'right' },
    { target: 'settings-tabs', key: 'settings.tabs', side: 'bottom' },
    { target: 'vault-security', key: 'settings.security', side: 'top' },
    { target: 'sync-add-device', key: 'settings.addDevice', side: 'top' },
    { target: 'sync-pending', key: 'settings.pending', side: 'bottom' },
    { target: 'sync-history', key: 'settings.history', side: 'top' },
    { target: 'sync-transfer', key: 'settings.transfer', side: 'top' },
    { target: 'learning-center', key: 'settings.learning', side: 'top' },
  ],
  // Before a vault is open: the card, the recent vaults, the sandbox.
  unlock: [
    { target: 'unlock-card', key: 'unlock.card', side: 'bottom' },
    { target: 'unlock-recents', key: 'unlock.recents', side: 'bottom' },
    { target: 'unlock-sandbox', key: 'unlock.sandbox', side: 'top' },
  ],
  stores: [
    { target: 'stores-list', key: 'stores.list', side: 'top' },
    { target: 'stores-create', key: 'stores.create', side: 'left' },
  ],
  wallet: [
    { target: 'wallet-actions', key: 'wallet.operation', side: 'bottom' },
    { target: 'wallet-gear', key: 'wallet.settings', side: 'bottom' },
    { target: 'wallet-sub-accounts', key: 'wallet.subAccounts', side: 'bottom' },
    { target: 'wallet-all-operations', key: 'wallet.allOperations', side: 'bottom' },
    { target: 'wallet-relations', key: 'wallet.relations', side: 'top' },
    { target: 'utility-reminders', key: 'wallet.utilityReminders', side: 'bottom' },
    { target: 'property-value', key: 'wallet.propertyValue', side: 'bottom' },
    { target: 'vehicle-card', key: 'wallet.vehicle', side: 'bottom' },
    { target: 'property-year', key: 'wallet.propertyYear', side: 'top' },
    { target: 'chart-currency', key: 'wallet.chartCurrency', side: 'left' },
    { target: 'wallet-positions', key: 'wallet.positions', side: 'top' },
    { target: 'wallet-tabs', key: 'wallet.views', side: 'bottom' },
    { target: 'exchange-terminal', key: 'wallet.terminal', side: 'top' },
  ],
  'wallet-settings': [
    { target: 'wallet-purpose', key: 'walletSettings.purpose', side: 'top' },
    { target: 'wallet-emergency', key: 'walletSettings.emergency', side: 'top' },
    { target: 'property-category', key: 'walletSettings.propertyType', side: 'bottom' },
    { target: 'property-car', key: 'walletSettings.car', side: 'top' },
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
    { target: 'tx-save-filter', key: 'transactions.saveFilter', side: 'bottom' },
    { target: 'section-content', key: 'transactions.list', side: 'top' },
    // Inside the operation dialog: shown when it is open, skipped otherwise.
    { target: 'tx-amount', key: 'transactions.calc', side: 'bottom' },
  ],
  'transactions-calendar': [
    { target: 'section-tabs', key: 'transactions.tabs', side: 'bottom' },
    { target: 'section-content', key: 'transactionsCalendar.content', side: 'top' },
    { target: 'calendar-balance', key: 'transactionsCalendar.balance', side: 'bottom' },
  ],
  money: analytics([
    { target: 'spending-answer', key: 'money.answer', side: 'bottom' },
    { target: 'period-picker', key: 'money.period', side: 'bottom' },
    { target: 'spending-months', key: 'money.months', side: 'top' },
    { target: 'spending-categories', key: 'money.categories', side: 'top' },
    { target: 'route-tabs-more', key: 'money.more', side: 'bottom' },
    { target: 'category-sheet', key: 'money.categorySheet', side: 'left' },
  ]),
  'money-where': analytics([
    { target: 'where-answer', key: 'money.whereAnswer', side: 'bottom' },
    { target: 'where-table', key: 'money.whereTable', side: 'top' },
    { target: 'report-view', key: 'analytics.flowView', side: 'bottom' },
    { target: 'real-terms', key: 'analytics.realTerms', side: 'bottom' },
    { target: 'section-content', key: 'analytics.flow', side: 'top' },
  ]),
  'money-wealth': analytics([
    { target: 'wealth-head', key: 'money.wealthHead', side: 'bottom' },
    { target: 'wealth-table', key: 'money.wealthTable', side: 'top' },
    { target: 'real-terms', key: 'analytics.realTerms', side: 'bottom' },
    { target: 'section-content', key: 'analytics.wealth', side: 'top' },
  ]),
  'money-investments': analytics([
    { target: 'invest-figures', key: 'money.investFigures', side: 'bottom' },
    { target: 'invest-table', key: 'money.investTable', side: 'top' },
    { target: 'section-content', key: 'analytics.investments', side: 'top' },
    { target: 'topup-advisor', key: 'assetClass.advisor', side: 'top' },
    { target: 'asset-classes', key: 'assetClass.classes', side: 'bottom' },
    { target: 'asset-baskets', key: 'assetClass.assets', side: 'top' },
  ]),
  'money-month': analytics([
    { target: 'section-content', key: 'analytics.saldo', side: 'top' },
    { target: 'saldo-pool', key: 'analytics.saldoPool', side: 'top' },
    { target: 'balance-days', key: 'money.balanceDays', side: 'top' },
    { target: 'balance-months', key: 'money.balanceMonths', side: 'top' },
    { target: 'carry-over', key: 'money.carryOver', side: 'left' },
  ]),
  // The checkup takes no filter and exports nothing: only the tabs, then its own cards.
  'money-health': [
    { target: 'section-tabs', key: 'analytics.tabs', side: 'bottom' },
    { target: 'health-figures', key: 'money.healthFigures', side: 'bottom' },
    { target: 'health-light', key: 'health.light', side: 'bottom' },
    { target: 'health-fund', key: 'health.fund', side: 'top' },
    { target: 'health-debts', key: 'health.debts', side: 'top' },
    { target: 'health-savings', key: 'health.savings', side: 'top' },
    { target: 'health-freedom', key: 'health.freedom', side: 'top' },
  ],
  'transactions-contacts': [
    { target: 'section-tabs', key: 'transactions.tabs', side: 'bottom' },
    { target: 'contact-quick-create', key: 'directory.create', side: 'bottom' },
    { target: 'contact-quick-filter', key: 'directory.quick', side: 'right' },
    { target: 'contact-groups', key: 'directory.groups', side: 'right' },
    { target: 'contact-editor', key: 'directory.editor', side: 'left' },
    { target: 'contact-accounts', key: 'directory.accounts', side: 'top' },
    { target: 'contact-debts', key: 'directory.debts', side: 'top' },
    { target: 'contact-transactions', key: 'directory.transactions', side: 'bottom' },
  ],
  'transactions-bonuses': [
    { target: 'section-tabs', key: 'transactions.tabs', side: 'bottom' },
    { target: 'bonuses-add', key: 'bonuses.add', side: 'left' },
    { target: 'bonuses-list', key: 'bonuses.list', side: 'top' },
    { target: 'bonuses-cashback', key: 'bonuses.cashback', side: 'top' },
  ],
  automations: workflows([
    { target: 'automations-create', key: 'automationsMine.create', side: 'bottom' },
    { target: 'automations-starts', key: 'automationsMine.starts', side: 'top' },
    { target: 'automations-mine', key: 'automationsMine.list', side: 'top' },
    { target: 'automations-import', key: 'automationsMine.import', side: 'bottom' },
  ]),
  'automations-sources': planning([
    { target: 'section-content', key: 'planningSources.content', side: 'top' },
    { target: 'sources-add', key: 'planningSources.add', side: 'bottom' },
    { target: 'sources-list', key: 'planningSources.list', side: 'top' },
    { target: 'source-url', key: 'planningSources.url', side: 'bottom' },
    { target: 'source-picker', key: 'planningSources.picker', side: 'top' },
    { target: 'source-method', key: 'planningSources.method', side: 'top' },
    { target: 'source-test', key: 'planningSources.test', side: 'top' },
    { target: 'source-uses', key: 'planningSources.uses', side: 'top' },
  ]),
  'money-plan': planning([
    { target: 'plan-limits', key: 'planningBudgets.content', side: 'top' },
    { target: 'budget-spent', key: 'planningBudgets.spent', side: 'bottom' },
    { target: 'plan-goals', key: 'planningGoals.content', side: 'top' },
    { target: 'plan-next', key: 'money.planNext', side: 'top' },
  ]),
  'money-calculator': planning([
    { target: 'calc-scenarios', key: 'planningCalculator.scenarios', side: 'bottom' },
    { target: 'calc-mode', key: 'planningCalculator.mode', side: 'bottom' },
    { target: 'calc-stages', key: 'planningCalculator.stages', side: 'top' },
    { target: 'calc-prefill', key: 'planningCalculator.prefill', side: 'top' },
    { target: 'calc-result', key: 'planningCalculator.result', side: 'left' },
    { target: 'calc-compare', key: 'planningCalculator.compare', side: 'top' },
    { target: 'calc-overlay', key: 'planningCalculator.overlay', side: 'top' },
    { target: 'calc-table', key: 'planningCalculator.table', side: 'top' },
  ]),
  'automations-reminders': planning([
    { target: 'section-content', key: 'planningReminders.content', side: 'top' },
    { target: 'reminder-filter', key: 'planningReminders.filter', side: 'bottom' },
  ]),
  'automations-alerts': planning([{ target: 'section-content', key: 'planningAlerts.content', side: 'top' }]),
}

/** Routes with no tour, and why (the `check-onboarding` gate reads this). */
export const TOURLESS_ROUTES: Record<string, string> = {
  'workflow-edit': 'The same editor as workflow-new; it shows that tour (TOUR_ALIASES).',
}

/** Routes that show another route's tour: a saved workflow opens the same editor as a new one. */
export const TOUR_ALIASES: Record<string, string> = {
  'workflow-edit': 'workflow-new',
}

/** The tour a route shows. */
export function tourIdOf(routeName: string): string {
  return TOUR_ALIASES[routeName] ?? routeName
}
