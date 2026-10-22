// What `mise run shots -- <set>` photographs. See steps.mjs for the shape of a shot.

const PALETTE = [{ desktop: [{ press: 'ctrl+k' }] }, { phone: [{ click: '[data-command-palette-trigger]' }] }]
const QUICK_ADD = [{ desktop: [{ click: '[data-tour="quick-add"]' }] }]
const COLLAPSE = [{ desktop: [{ press: 'ctrl+b' }] }]
/** Ctrl B again: the sidebar state is remembered, the next shot starts expanded. */
const EXPAND = { name: 'sidebar-restore', route: '/', steps: COLLAPSE, phone: false }

const ALL = ['year', 'empty', 'stress', 'slow', 'error']

/** The interface as it is before the redesign: what the new one is compared with. */
const before = [
  { name: 'home', route: '/', scenarios: ALL, canvas: { board: 'Home', frame: '1.' } },
  { name: 'stores', route: '/stores', scenarios: ['year', 'empty', 'stress'] },
  { name: 'transactions', route: '/transactions', scenarios: ['year', 'empty'], canvas: { board: 'Ops-1', frame: '1.' } },
  { name: 'calendar', route: '/transactions/calendar', canvas: { board: 'Ops-1', frame: '5.' } },
  { name: 'flow', route: '/analytics/flow', scenarios: ['year', 'stress'], canvas: { board: 'Money-1', frame: '1.' } },
  { name: 'saldo', route: '/analytics/saldo', canvas: { board: 'Money-3', frame: '15.' } },
  { name: 'wealth', route: '/analytics/wealth' },
  { name: 'budgets', route: '/planning/budgets' },
  { name: 'automations', route: '/planning/automations', scenarios: ['year', 'empty'] },
  { name: 'palette', route: '/', steps: PALETTE, canvas: { board: 'Shell', frame: '4.' } },
  { name: 'quick-add', route: '/', steps: QUICK_ADD, phone: false },
  { name: 'sidebar-collapsed', route: '/', steps: COLLAPSE, phone: false, canvas: { board: 'Shell', frame: '7.' } },
  EXPAND,
  { name: 'settings', route: 'settings:general' },
]

/** Stage 1: the shell, Money › Spending, the month balance, the period. */
const MORE = [{ phone: [{ click: '[data-bottom-more]' }] }]
const PERIOD = [{ click: '[data-period-trigger]' }]
const stage1 = [
  { name: 'home', route: '/', scenarios: ['year', 'empty'] },
  { name: 'spending', route: '/money', scenarios: ALL, canvas: { board: 'Money-1', frame: '1.' } },
  { name: 'period', route: '/money', steps: PERIOD, canvas: { board: 'Period', frame: '1.' } },
  { name: 'month', route: '/money/month', scenarios: ['year', 'empty', 'stress'], canvas: { board: 'Money-3', frame: '15.' } },
  { name: 'money-more', route: '/money', steps: [{ click: '[data-tour="route-tabs-more"]' }] },
  { name: 'automations', route: '/automations', scenarios: ['year', 'empty'] },
  { name: 'palette', route: '/money', steps: PALETTE, canvas: { board: 'Shell', frame: '4.' } },
  { name: 'more-sheet', route: '/', steps: MORE, desktop: false, canvas: { board: 'Shell', frame: '6.' } },
  { name: 'sidebar-collapsed', route: '/money', steps: COLLAPSE, phone: false, canvas: { board: 'Shell', frame: '7.' } },
  EXPAND,
]

/** Stage 2a: Home — three figures, things to do, what changed, the widget catalog, first steps. */
const CATALOG = [{ click: '[data-tour="dashboard-add"]' }]
const stage2 = [
  { name: 'home', route: '/', scenarios: ALL, canvas: { board: 'Home', frame: '1.' } },
  { name: 'home-first', route: '/', scenarios: ['empty'], canvas: { board: 'Home', frame: '5.' } },
  { name: 'home-catalog', route: '/', steps: CATALOG, canvas: { board: 'Home', frame: '4.' } },
]

export const SETS = { before, stage1, stage2 }
