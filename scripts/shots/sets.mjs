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

export const SETS = { before }
