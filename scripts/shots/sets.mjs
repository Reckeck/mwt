// What `mise run shots -- <set>` photographs. See steps.mjs for the shape of a shot.

const PALETTE = [{ desktop: [{ press: 'ctrl+k' }] }, { phone: [{ click: '[data-command-palette-trigger]' }] }]
const QUICK_ADD = [{ desktop: [{ click: '[data-tour="quick-add"]' }] }]
const COLLAPSE = [{ desktop: [{ press: 'ctrl+b' }] }]
/** Ctrl B again: the sidebar state is remembered, the next shot starts expanded. */
const EXPAND = { name: 'sidebar-restore', route: '/', steps: COLLAPSE, phone: false }

/** «Недавние» as on Paths 1 · 7: a wallet, the October spending and an operation card were open a moment ago. */
const SEED_RECENT = `
  const gp = document.querySelector('#app').__vue_app__.config.globalProperties
  const wait = () => new Promise((done) => setTimeout(done, 600))
  await gp.$router.push('/money?from=2026-10&to=2026-10'); await wait()
  await gp.$router.push('/stores'); await wait()
  const wallet = document.querySelector('a[href^="/stores/"]')?.getAttribute('href')
  if (wallet) { await gp.$router.push(wallet); await wait() }
  await gp.$router.push('/'); await wait(); return true`

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

/** Stage 2b: the rest of Money (canvas Money-1…4) and the paths (Paths-1, 2). */
const CATEGORY = [{ click: '[data-category-row]' }]
const LIMIT = [{ click: '[data-category-limit]' }]
const LIMIT_NEXT = [...LIMIT, { click: '[data-wizard-next]' }, { fill: ['#budget-limit', '300'] }]
const GOAL = [{ click: '[data-where-goal]' }]
const GOAL_NEXT = [...GOAL, { fill: ['#goal-name', 'Подушка безопасности'] }, { fill: ['#goal-target', '5000'] }, { click: '[data-wizard-next]' }]
const stage2b = [
  { name: 'category', route: '/money', steps: CATEGORY, scenarios: ['year', 'stress'], canvas: { board: 'Money-1', frame: '2.' } },
  { name: 'limit-1', route: '/money', steps: LIMIT, canvas: { board: 'Money-1', frame: '3.' } },
  { name: 'limit-2', route: '/money', steps: LIMIT_NEXT, canvas: { board: 'Money-1', frame: '4.' } },
  { name: 'limit-saved', route: '/money', steps: [...LIMIT_NEXT, { click: '[data-wizard-save]' }], canvas: { board: 'Money-1', frame: '5.' } },
  { name: 'where', route: '/money/where', scenarios: ['year', 'empty', 'stress'], canvas: { board: 'Money-1', frame: '6.' } },
  { name: 'goal-1', route: '/money/where', steps: GOAL, canvas: { board: 'Money-1', frame: '7.' } },
  { name: 'goal-1-errors', route: '/money/where', steps: [...GOAL, { click: '[data-wizard-next]' }], canvas: { board: 'Money-1', frame: '7.' } },
  { name: 'goal-2', route: '/money/where', steps: GOAL_NEXT, canvas: { board: 'Money-2', frame: '8.' } },
  { name: 'plan', route: '/money/plan', scenarios: ['year', 'empty', 'stress'], canvas: { board: 'Money-2', frame: '10.' } },
  {
    name: 'plan-full',
    route: '/money/plan',
    steps: [{ click: '[data-plan-limit-top]' }, { click: '[data-wizard-next]' }, { fill: ['#budget-limit', '500'] }, { click: '[data-wizard-save]' }],
    canvas: { board: 'Money-2', frame: '10.' },
  },
  { name: 'more-menu', route: '/money', steps: [{ click: '[data-route-tabs-more]' }], canvas: { board: 'Money-2', frame: '11.' } },
  { name: 'wealth', route: '/money/wealth', scenarios: ['year', 'empty', 'stress'], canvas: { board: 'Money-2', frame: '12.' } },
  { name: 'health', route: '/money/health', scenarios: ['year', 'empty'], canvas: { board: 'Money-3', frame: '16.' } },
  {
    name: 'fund-setup',
    route: '/money/health',
    steps: [{ click: '[data-health-fund-setup]' }, { wait: 400 }],
    canvas: { board: 'Money-3', frame: '17.' },
  },
  { name: 'calc', route: '/money/calculator', steps: [{ wait: 1500 }], canvas: { board: 'Money-3', frame: '18.' } },
  {
    name: 'calc-compare',
    route: '/money/calculator',
    steps: [
      { wait: 1200 },
      { click: '[data-calc-compare]' },
      { click: '[data-calc-compare-menu] label:nth-of-type(1)' },
      { click: '[data-calc-compare-menu] label:nth-of-type(3)' },
      { press: 'Escape' },
      { wait: 1200 },
      { phone: [{ hover: '[data-calc-compare-list]' }] },
    ],
    canvas: { board: 'Money-4', frame: '22.' },
  },
  {
    name: 'calc-compare-menu',
    route: '/money/calculator',
    steps: [{ wait: 1200 }, { click: '[data-calc-compare]' }, { click: '[data-calc-compare-menu] label:nth-of-type(1)' }, { wait: 800 }],
    canvas: { board: 'Money-4', frame: '22.' },
  },
  {
    name: 'calc-contribution',
    route: '/money/calculator',
    steps: [{ click: '[data-calc-mode="contribution"]' }, { fill: ['#calc-target', '20000'] }, { wait: 900 }],
    canvas: { board: 'Money-3', frame: '19.' },
  },
  {
    name: 'calc-prefill',
    route: '/money/calculator',
    steps: [{ click: '[data-calc-prefill] button' }, { wait: 300 }],
    phone: false,
  },
  {
    name: 'calc-withdrawal',
    route: '/money/calculator',
    steps: [
      { click: '[data-calc-advanced] button' },
      { click: '#calc-withdrawal-on' },
      { fill: ['#calc-withdrawal', '30000'] },
      { wait: 1200 },
    ],
    canvas: { board: 'Money-3', frame: '21.' },
  },
  { name: 'calc-goal', route: '/money/calculator', steps: [{ click: '[data-calc-goal]' }, { wait: 400 }], phone: false },
  { name: 'investments', route: '/money/investments', scenarios: ['year', 'empty'], canvas: { board: 'Money-2', frame: '13.' } },
  {
    name: 'trade-record',
    route: '/money/investments',
    steps: [{ click: '[data-invest-record]' }, { wait: 400 }],
    canvas: { board: 'Money-2', frame: '14.' },
  },
  {
    name: 'trade-record-errors',
    route: '/money/investments',
    steps: [{ click: '[data-invest-record]' }, { wait: 400 }, { click: 'button[form="trade-record"]' }, { wait: 200 }],
    phone: false,
  },
  {
    name: 'trade-record-filled',
    route: '/money/investments',
    steps: [
      { click: '[data-invest-record]' },
      { wait: 400 },
      { click: '[data-trade-side="sell"]' },
      { fill: ['#trade-qty', '0,05'] },
      { fill: ['#trade-price', '90 000'] },
    ],
    phone: false,
  },
  {
    name: 'trade-calc',
    route: '/money/investments',
    steps: [
      { click: '[data-invest-calc]' },
      { wait: 400 },
      { type: ['[data-calc-asset] input', 'btc'] },
      { wait: 1200 },
      { click: '[data-slot="combobox-item"]' },
      { wait: 600 },
      { fill: ['#calc-amount', '1000'] },
      { fill: ['#calc-target', '100000'] },
      { fill: ['#calc-stop', '85000'] },
      { wait: 900 },
    ],
    canvas: { board: 'Money-4', frame: '23.' },
  },
  { name: 'trade-calc-empty', route: '/money/investments', steps: [{ click: '[data-invest-calc]' }, { wait: 400 }], phone: false },
  { name: 'goal-2-errors', route: '/money/where', steps: [...GOAL_NEXT, { click: '[data-wizard-save]' }], canvas: { board: 'Money-2', frame: '8.' } },
]

/** Paths 1–2: glances, the way back, the action menu. */
const paths = [
  { name: 'glance-spent', route: '/', steps: [{ wait: 600 }, { click: '[data-glance-trigger="spent"]' }, { wait: 900 }], canvas: { board: 'Paths-1', frame: '2.' } },
  { name: 'glance-wallet', route: '/transactions', steps: [{ wait: 800 }, { click: '[data-glance-trigger="wallet"]' }, { wait: 900 }], canvas: { board: 'Paths-1', frame: '4.' } },
  {
    name: 'back-chip',
    route: '/',
    steps: [{ wait: 600 }, { click: '[data-glance-trigger="spent"]' }, { wait: 900 }, { click: '[data-glance-category]' }, { wait: 1200 }],
    canvas: { board: 'Paths-1', frame: '5.' },
  },
  { name: 'tx-card', route: '/transactions', steps: [{ wait: 800 }, { eval: `document.querySelector('[data-tx-open]').click(); return true` }, { wait: 900 }], canvas: { board: 'Paths-1', frame: '6.' } },
  {
    name: 'tx-card-wallet',
    route: '/transactions',
    steps: [{ wait: 800 }, { eval: `document.querySelector('[data-tx-open]').click(); return true` }, { wait: 900 }, { click: '[data-tx-facts] [data-glance-trigger="wallet"]' }, { wait: 700 }],
    canvas: { board: 'Paths-1', frame: '6.' },
  },
  { name: 'palette-empty', route: '/', steps: [{ wait: 600 }, { eval: SEED_RECENT }, ...PALETTE, { wait: 600 }], canvas: { board: 'Paths-1', frame: '7.' } },
  {
    name: 'palette-peek',
    route: '/',
    steps: [{ wait: 600 }, ...PALETTE, { wait: 300 }, { type: ['[data-command-palette] input', 'прод'] }, { wait: 1200 }],
    canvas: { board: 'Paths-2', frame: '8.' },
  },
  { name: 'palette-shift', route: '/', steps: [{ wait: 600 }, { eval: SEED_RECENT }, ...PALETTE, { wait: 300 }, { press: 'shift+Enter' }, { wait: 900 }], phone: false },
  {
    name: 'glance-auto',
    route: '/transactions',
    steps: [{ wait: 800 }, { click: '[data-glance-trigger="auto"]' }, { wait: 900 }],
    canvas: { board: 'Paths-2', frame: '10.' },
  },
  {
    name: 'where-open',
    route: '/money/where',
    steps: [{ wait: 900 }, { eval: `document.querySelector('[data-where-row="moved"] button').click(); return true` }, { wait: 600 }],
    canvas: { board: 'Paths-2', frame: '9.' },
  },
  {
    name: 'row-menu',
    route: '/transactions',
    steps: [{ wait: 900 }, { click: 'section:nth-of-type(2) [data-tx-row] button[aria-label="Ещё"]' }, { wait: 700 }],
    canvas: { board: 'Paths-2', frame: '11.' },
  },
  { name: 'glance-capital', route: '/', steps: [{ wait: 600 }, { click: '[data-glance-trigger="capital"]' }, { wait: 900 }], canvas: { board: 'Paths-1', frame: '3.' } },
]

/** Stage 3: operations (canvas Ops-1, 2), the operation forms (OpForm-1, 2), attachments and the goal (New-Attach, New-Goal). */
/** Two rows picked: checkboxes on a desktop, a hold → «Выбрать» and a tap on a phone. */
const HOLD_FIRST_ROW = `const row = document.querySelectorAll('[data-tx-open]')[0]
  const at = { bubbles: true, pointerType: 'touch', isPrimary: true }
  row.dispatchEvent(new PointerEvent('pointerdown', at))
  await new Promise((done) => setTimeout(done, 650))
  row.dispatchEvent(new PointerEvent('pointerup', at))
  return true`
const PICK_TWO = [
  { desktop: [{ eval: `document.querySelectorAll('[data-tx-select]')[0].click(); document.querySelectorAll('[data-tx-select]')[2].click(); return true` }] },
  { phone: [{ eval: HOLD_FIRST_ROW }, { wait: 400 }, { click: { text: 'Выбрать', within: '[role="dialog"]' } }, { wait: 300 }, { eval: `document.querySelectorAll('[data-tx-open]')[2].click(); return true` }] },
  { wait: 300 },
]

const stage3 = [
  { name: 'ops', route: '/transactions', scenarios: ['year', 'empty', 'stress'], canvas: { board: 'Ops-1', frame: '1.' } },
  { name: 'ops-more', route: '/transactions', steps: [{ wait: 600 }, { click: '[data-filter-more]' }, { wait: 500 }], canvas: { board: 'Ops-1', frame: '2.' } },
  { name: 'ops-select', route: '/transactions', steps: [{ wait: 600 }, ...PICK_TWO], canvas: { board: 'Ops-1', frame: '4.' } },
  { name: 'ops-select-category', route: '/transactions', steps: [{ wait: 600 }, ...PICK_TWO, { desktop: [{ click: { text: 'Категория', within: '[data-tx-bulk]' } }] }, { phone: [{ click: '[data-tx-bulk] button[aria-label="Изменить"]' }, { wait: 300 }, { click: { text: 'Категория', within: '[role="dialog"]' } }] }, { wait: 400 }] },
  { name: 'ops-select-deleted', route: '/transactions', steps: [{ wait: 600 }, ...PICK_TWO, { desktop: [{ click: { text: 'Удалить', within: '[data-tx-bulk]' } }] }, { phone: [{ click: '[data-tx-bulk] button[aria-label="Удалить"]' }] }, { wait: 600 }] },
  { name: 'ops-calendar', route: '/transactions/calendar', canvas: { board: 'Ops-1', frame: '5.' } },
  { name: 'ops-contacts', route: '/transactions/contacts', steps: [{ wait: 600 }], canvas: { board: 'Ops-1', frame: '6.' } },
  { name: 'ops-bonuses', route: '/transactions/bonuses', canvas: { board: 'Ops-2', frame: '12.' } },
  { name: 'ops-contact-menu', route: '/transactions/contacts?contact=mock-contact-mom', steps: [{ desktop: [{ click: '[data-contact-account] button[aria-label="Ещё"]' }] }], canvas: { board: 'Ops-2', frame: '7.' } },
  {
    name: 'ops-contact-paste',
    route: '/transactions/contacts?contact=mock-contact-mom',
    steps: [{ click: '[data-contact-paste]' }, { fill: ['#contact-account-value', '2202 2061 3348 7798'] }, { wait: 300 }],
    canvas: { board: 'Ops-2', frame: '8.' } },
  {
    name: 'ops-contact-copied',
    route: '/transactions/contacts?contact=mock-contact-mom',
    steps: [{ click: '[data-contact-copy]' }, { wait: 300 }],
    canvas: { board: 'Ops-2', frame: '9.' } },
  {
    name: 'ops-bonus-new',
    route: '/transactions/bonuses',
    steps: [{ wait: 400 }, { click: '[data-tour="bonuses-add"]' }, { fill: ['#program-name', 'Тинькофф кешбэк'] }, { wait: 300 }],
    canvas: { board: 'Ops-2', frame: '13.' },
  },
  {
    name: 'ops-bonus-step2',
    route: '/transactions/bonuses',
    steps: [
      { wait: 400 },
      { click: '[data-tour="bonuses-add"]' },
      { fill: ['#program-name', 'Аэрофлот'] },
      { click: '#program-wallet' },
      { wait: 200 },
      { click: '[role="option"]' },
      { click: '[data-wizard-next]' },
      { wait: 300 },
    ],
    canvas: { board: 'Ops-2', frame: '13.' },
  },
  { name: 'opform-pay', route: '/', steps: [{ wait: 600 }, { press: 'ctrl+n' }, { wait: 700 }], canvas: { board: 'OpForm-1', frame: '1.' } },
]

export const SETS = { before, stage1, stage2, stage2b, paths, stage3 }
