// What `mise run shots -- <set>` photographs. See steps.mjs for the shape of a shot.

import { stage5 } from './sets-stage5.mjs'
import { links } from './sets-links.mjs'
import { money, moneyScreens } from './sets-money.mjs'
import { encrypt } from './sets-encrypt.mjs'
import { stage6 } from './sets-stage6.mjs'
import { samecur } from './sets-samecur.mjs'
import { GP } from './steps.mjs'

/** The device goes offline: `navigator.onLine` false and the event `useOnline` listens to. */
const OFFLINE = `Object.defineProperty(navigator, 'onLine', { configurable: true, get: () => false }); window.dispatchEvent(new Event('offline')); return true`
/** Back online: the override above outlives a route change, the shots do not reload the page. */
const ONLINE = `delete navigator.onLine; window.dispatchEvent(new Event('online')); return true`
const PALETTE = [{ desktop: [{ press: 'ctrl+k' }] }, { phone: [{ click: '[data-command-palette-trigger]' }] }]
const QUICK_ADD = [{ desktop: [{ click: '[data-tour="quick-add"]' }] }]
const COLLAPSE = [{ desktop: [{ press: 'ctrl+b' }] }]
/** Ctrl B again: the sidebar state is remembered, the next shot starts expanded. */
const EXPAND = { name: 'sidebar-restore', route: '/', steps: COLLAPSE, phone: false }

/** «Недавние» as on Paths 1 · 7: a wallet, the October spending and an operation card were open a moment ago. */
const SEED_RECENT = `
  ${GP}
  const wait = () => new Promise((done) => setTimeout(done, 600))
  await gp.$router.push('/money?from=2026-10&to=2026-10'); await wait()
  await gp.$router.push('/stores'); await wait()
  const wallet = document.querySelector('a[href^="/stores/"]')?.getAttribute('href')
  if (wallet) { await gp.$router.push(wallet); await wait() }
  await gp.$router.push('/'); await wait(); return true`

/** The Bybit wallet's own page: «Сделка · Bybit» opens from its header (OpForm-2 · 9–11). */
const TO_BYBIT = `
  ${GP}
  await gp.$router.push('/stores'); await new Promise((done) => setTimeout(done, 700))
  const link = [...document.querySelectorAll('a[href^="/stores/"]')].find((a) => a.textContent.includes('Bybit'))
  if (link) await gp.$router.push(link.getAttribute('href'))
  await new Promise((done) => setTimeout(done, 900)); return true`
/** A wallet's own page by the name in the wallets list. */
const toWallet = (name, tab = '') => `
  ${GP}
  await gp.$router.push('/stores'); await new Promise((done) => setTimeout(done, 700))
  const link = [...document.querySelectorAll('a[href^="/stores/"]')].find((a) => a.textContent.includes('${name}'))
  if (link) await gp.$router.push(link.getAttribute('href') + '${tab ? `?tab=${tab}` : ''}')
  await new Promise((done) => setTimeout(done, 900)); return true`
/** Opens an item of the home's bill by its name (Wallets-2 · 11–12). */
const openLine = (name) => `
  const rows = [...document.querySelectorAll('[data-utility-lines] tr[data-utility-line], [data-utility-lines] li > button')]
  rows.find((row) => row.textContent.includes('${name}'))?.click()
  await new Promise((done) => setTimeout(done, 900)); return true`
/** «Оплатить счёт» on the home's bill card (Wallets-2 · 13); the visible one of the two layouts. */
const PAY_BILL = `
  ;[...document.querySelectorAll('[data-bill-pay]')].find((button) => button.offsetParent)?.click()
  await new Promise((done) => setTimeout(done, 900)); return true`
/** «+ Статья» on the property's bill tab (Wallets-3 · 14); the visible one of the two layouts. */
const NEW_ITEM = `
  ;[...document.querySelectorAll('[data-utility-new]')].find((button) => button.offsetParent)?.click()
  await new Promise((done) => setTimeout(done, 700)); return true`
/** The item wizard on step 2 for a name and a kind card (2 fixed, 3 rent; a meter is picked already). */
const itemStep2 = (name, card = 0) => [
  { eval: toWallet('Квартира') },
  { eval: NEW_ITEM },
  { fill: ['#utility-name', name] },
  ...(card ? [{ click: `[data-utility-kinds] button:nth-child(${card})` }] : []),
  { click: '[data-wizard-next]' },
  { wait: 500 },
]
/** A wallet's gear menu item (Wallets-3 · 19–20). */
const gear = (item) => [{ click: '[data-tour="wallet-gear"]' }, { wait: 300 }, { click: `[data-gear="${item}"]` }, { wait: 600 }]
const deal = (kind, fills) => [{ eval: TO_BYBIT }, { click: `[data-deal="${kind}"]` }, { wait: 800 }, ...fills.map((fill) => ({ fill })), { wait: 900 }]

const ALL = ['year', 'empty', 'stress', 'slow', 'error']

/** The interface as it is before the redesign: what the new one is compared with. */
const before = [
  { name: 'home', route: '/', scenarios: ALL, canvas: { board: 'Home', frame: '1.' } },
  { name: 'stores', route: '/stores', scenarios: ['year', 'empty', 'stress'] },
  { name: 'transactions', route: '/transactions', scenarios: ['year', 'empty'], canvas: { board: 'Ops-1', frame: '1.' } },
  { name: 'calendar', route: '/transactions/calendar', canvas: { board: 'Ops-1', frame: '5.' } },
  { name: 'flow', route: '/money/where', scenarios: ['year', 'stress'], canvas: { board: 'Money-1', frame: '1.' } },
  { name: 'saldo', route: '/money/month', canvas: { board: 'Money-3', frame: '15.' } },
  { name: 'wealth', route: '/money/wealth' },
  { name: 'budgets', route: '/money/plan' },
  { name: 'automations', route: '/automations', scenarios: ['year', 'empty'] },
  { name: 'palette', route: '/', steps: PALETTE, canvas: { board: 'Shell', frame: '4.' } },
  { name: 'quick-add', route: '/', steps: QUICK_ADD, phone: false },
  { name: 'sidebar-collapsed', route: '/', steps: COLLAPSE, phone: false, canvas: { board: 'Shell', frame: '7.' } },
  EXPAND,
  { name: 'settings', route: 'settings:general' },
]

/** Stage 1: the shell, Money › Spending, the month balance, the period. */
const MORE = [{ phone: [{ click: '[data-bottom-more]' }] }]
const PERIOD = [{ click: '[data-period-trigger]' }]
/** The mock keeps its settings between shots of a run: run these two as separate `--only` runs, each starts from «off» and presses the switch. */
const CARRY_ON = [
  { eval: `const data = await import('/src/shared/api/mock/vault-data.ts'); data.mockSettings.carry_since = null; return true` },
  { eval: `${GP} await gp.$router.push('/'); await gp.$router.push('/money/month'); return true` },
  { wait: 900 },
  { click: '#carry-over' },
  { wait: 900 },
]
const stage1 = [
  { name: 'home', route: '/', scenarios: ['year', 'empty'] },
  { name: 'spending', route: '/money', scenarios: ALL, canvas: { board: 'Money-1', frame: '1.' } },
  { name: 'period', route: '/money', steps: PERIOD, canvas: { board: 'Period', frame: '1.' } },
  { name: 'month', route: '/money/month', scenarios: ['year', 'empty', 'stress'], canvas: { board: 'Money-3', frame: '15.' } },
  // The carry switched on with the real switch (it refreshes the queries): the line «Перенесено дальше» in «По месяцам», «всего с сентября» in the answer.
  { name: 'month-carry', route: '/money/month', steps: [...CARRY_ON, { wait: 900 }] },
  { name: 'spending-carry', route: '/money/month', steps: [...CARRY_ON, { eval: `${GP} await gp.$router.push('/money'); return true` }, { wait: 900 }] },
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
const GOAL_NEXT = [...GOAL, { fill: ['#goal-name', 'Подушка безопасности'] }, { fill: ['#goal-target', '5000'] }]
const stage2b = [
  { name: 'category', route: '/money', steps: CATEGORY, scenarios: ['year', 'stress'], canvas: { board: 'Money-1', frame: '2.' } },
  { name: 'limit-1', route: '/money', steps: LIMIT, canvas: { board: 'Money-1', frame: '3.' } },
  { name: 'limit-2', route: '/money', steps: LIMIT_NEXT, canvas: { board: 'Money-1', frame: '4.' } },
  { name: 'limit-saved', route: '/money', steps: [...LIMIT_NEXT, { click: '[data-wizard-save]' }], canvas: { board: 'Money-1', frame: '5.' } },
  { name: 'where', route: '/money/where', scenarios: ['year', 'empty', 'stress'], canvas: { board: 'Money-1', frame: '6.' } },
  { name: 'goal-1', route: '/money/where', steps: GOAL, canvas: { board: 'Money-1', frame: '7.' } },
  { name: 'goal-1-errors', route: '/money/where', steps: [...GOAL, { click: '[data-wizard-save]' }], canvas: { board: 'Money-1', frame: '7.' } },
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

/** Opens the first operation that already has files (the mock puts them on the two latest payments). */
const OPEN_ATTACHED = {
  eval: `for (const row of document.querySelectorAll('[data-tx-open]')) {
    row.click()
    await new Promise((done) => setTimeout(done, 500))
    if (document.querySelector('[data-attachments] [data-attachment-row]')) return true
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await new Promise((done) => setTimeout(done, 300))
  }
  return false`,
}
/** Opens an operation without files: the oldest one on screen. */
const OPEN_BARE = { eval: `[...document.querySelectorAll('[data-tx-open]')].at(-1).click(); return true` }
const ATTACH = (frame) => ({ board: 'New-Attach', frame })

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
  {
    name: 'opform-more',
    route: '/',
    steps: [{ wait: 600 }, { press: 'ctrl+n' }, { wait: 700 }, { click: '[data-tour="tx-more"] > button' }, { wait: 400 }],
    canvas: { board: 'OpForm-1', frame: '5.' },
  },
  {
    name: 'opform-errors',
    route: '/',
    steps: [{ wait: 600 }, { press: 'ctrl+n' }, { wait: 700 }, { click: '[data-quick-add-submit]' }, { wait: 400 }],
    canvas: { board: 'OpForm-1', frame: '3.' },
  },
  {
    name: 'opform-series',
    route: '/',
    steps: [
      { wait: 600 },
      { press: 'ctrl+n' },
      { wait: 700 },
      { wait: 800 },
      { fill: ['#tx-amount', '340'] },
      { click: '[data-frequent-category]' },
      { press: 'ctrl+enter' },
      { wait: 600 },
      { fill: ['#tx-amount', '120'] },
      { click: '[data-frequent-category]' },
      { press: 'ctrl+enter' },
      { wait: 800 },
    ],
    canvas: { board: 'OpForm-1', frame: '4.' },
  },
  {
    name: 'opform-convert',
    route: '/',
    steps: [{ wait: 600 }, { press: 'ctrl+n' }, { wait: 700 }, { click: '[data-kind-tab="convert"]' }, { wait: 600 }, { fill: ['#tx-amount', '45000'] }, { fill: ['#tx-to-amount', '480'] }, { wait: 300 }],
    canvas: { board: 'OpForm-1', frame: '6.' },
  },
  {
    name: 'opform-transfer',
    route: '/',
    steps: [{ wait: 600 }, { press: 'ctrl+n' }, { wait: 700 }, { click: '[data-kind-tab="transfer"]' }, { wait: 600 }, { fill: ['#tx-amount', '5000'] }, { wait: 300 }],
    canvas: { board: 'OpForm-1', frame: '7.' },
  },
  {
    name: 'opform-topup',
    route: '/',
    steps: [{ wait: 600 }, { press: 'ctrl+n' }, { wait: 700 }, { click: '[data-kind-tab="topup"]' }, { wait: 600 }],
    canvas: { board: 'OpForm-2', frame: '8.' },
  },
  {
    name: 'deal-spot',
    route: '/',
    steps: deal('trade_spot', [['#tx-pair', 'BTCUSDT'], ['#tx-amount', '0.0143'], ['#tx-spot-price', '89900'], ['#tx-fee', '1.29']]),
    canvas: { board: 'OpForm-2', frame: '9.' },
  },
  {
    name: 'deal-futures',
    route: '/',
    steps: deal('trade_futures', [['#tx-symbol', 'BTCUSDT'], ['#tx-amount', '400'], ['#tx-fill', '89552']]),
    canvas: { board: 'OpForm-2', frame: '10.' },
  },
  {
    name: 'deal-defi',
    route: '/',
    steps: deal('defi', [['#tx-protocol', 'Bybit Earn'], ['#tx-amount', '200']]),
    canvas: { board: 'OpForm-2', frame: '11.' },
  },
  { name: 'attach-card', route: '/transactions', steps: [{ wait: 800 }, OPEN_ATTACHED, { wait: 600 }, { eval: `document.querySelector('[data-attachments]').scrollIntoView({ block: 'center' }); return true` }, { wait: 300 }], canvas: ATTACH('1.') },
  { name: 'attach-empty', route: '/transactions', steps: [{ wait: 800 }, OPEN_BARE, { wait: 900 }, { eval: `document.querySelector('[data-attachments]').scrollIntoView({ block: 'center' }); return true` }, { wait: 300 }], canvas: ATTACH('2.') },
  { name: 'attach-add', route: '/transactions', steps: [{ wait: 800 }, OPEN_BARE, { wait: 900 }, { click: '[data-attachment-add]' }, { wait: 500 }], canvas: ATTACH('2.') },
  {
    name: 'attach-link',
    route: '/transactions',
    steps: [{ wait: 800 }, OPEN_ATTACHED, { wait: 600 }, { click: '[data-attachment-add]' }, { wait: 400 }, { click: '[data-attachment-pick="link"]' }, { wait: 500 }, { fill: ['#attachment-link-url', 't.me/c/1848/2231'] }, { wait: 200 }],
    canvas: ATTACH('4.'),
  },
  { name: 'attach-list', route: '/transactions', steps: [{ wait: 600 }, { click: '[data-filter-more]' }, { wait: 500 }, { eval: `document.querySelector('[id$="-files"]')?.click(); return true` }, { wait: 700 }], canvas: ATTACH('6.') },
  { name: 'attach-filtered', route: '/transactions?files=1', steps: [{ wait: 900 }], canvas: ATTACH('6.') },
  {
    name: 'attach-form',
    route: '/',
    steps: [
      { wait: 600 },
      { press: 'ctrl+n' },
      { wait: 700 },
      { click: '[data-tour="tx-more"] > button' },
      { wait: 400 },
      { click: '[data-attachments-field] [data-attachment-add]' },
      { wait: 400 },
      { click: '[data-attachment-pick="link"]' },
      { wait: 500 },
      { fill: ['#attachment-link-url', 'https://ofd.ru/check/8841'] },
      { fill: ['#attachment-link-title', 'Чек.jpg'] },
      { click: '[data-attachment-link-form] [type="submit"]' },
      { wait: 500 },
    ],
    canvas: ATTACH('5.'),
  },
  {
    name: 'attach-remove',
    route: '/transactions',
    steps: [
      { wait: 600 },
      OPEN_ATTACHED,
      {
        eval: `const dt = new DataTransfer()
    dt.items.add(new File([new Uint8Array(31 * 1024 * 1024)], 'Договор обмена.pdf', { type: 'application/pdf' }))
    document.querySelector('[data-attachments]').dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true, cancelable: true }))
    return true`,
      },
      { wait: 400 },
      { click: '[data-attachments] [data-attachment-row] button[aria-label="Ещё"]' },
      { wait: 400 },
      {
        eval: `const item = [...document.querySelectorAll('[role="menuitem"], [data-attachment-sheet] button, [role="dialog"] button')].findLast((el) => el.textContent.trim() === 'Удалить')
    item?.click()
    return Boolean(item)`,
      },
      { wait: 800 },
    ],
    canvas: ATTACH('7.'),
  },
  { name: 'goal-new-errors', route: '/money/plan', steps: [{ click: '[data-plan-goal]' }, { wait: 300 }, { fill: ['#goal-target', '0'] }, { click: '[data-wizard-save]' }, { wait: 300 }], canvas: { board: 'New-Goal', frame: '2.' } },
  { name: 'goal-card', route: '/money/plan', steps: [{ click: '[data-entity^="goal:"]' }, { wait: 500 }], canvas: { board: 'New-Goal', frame: '3.' } },
  { name: 'goal-delete', route: '/money/plan', steps: [{ click: '[data-entity^="goal:"]' }, { wait: 500 }, { click: '[data-goal-delete]' }, { wait: 400 }], canvas: { board: 'New-Goal', frame: '5.' } },
  { name: 'goal-edit', route: '/money/plan', steps: [{ click: '[data-entity^="goal:"]' }, { wait: 500 }, { click: '[data-goal-edit]' }, { wait: 500 }], canvas: { board: 'New-Goal', frame: '4.' } },
]

/** Opens «Новый кошелёк» from the wallets header. */
const CREATE_STORE = [{ wait: 600 }, { click: '[data-tour="stores-create"]' }, { wait: 500 }]

/** Stage 4: wallets and currencies. */
const stage4 = [
  { name: 'stores', route: '/stores', scenarios: ['year', 'empty'], canvas: { board: 'Wallets-1', frame: '1.' } },
  { name: 'stores-currency', route: '/stores', canvas: { board: 'Currency-1', frame: '2.' } },
  { name: 'wallet-bank', route: '/stores', steps: [{ eval: toWallet('Тинькофф') }], canvas: { board: 'Wallets-1', frame: '5.' } },
  { name: 'wallet-crypto', route: '/stores', steps: [{ eval: toWallet('Ledger') }], canvas: { board: 'Wallets-1', frame: '6.' } },
  { name: 'wallet-exchange', route: '/stores', steps: [{ eval: toWallet('Bybit') }], canvas: { board: 'Wallets-1', frame: '7.' } },
  { name: 'wallet-exchange-import', route: '/stores', steps: [{ eval: toWallet('Bybit', 'import') }], canvas: { board: 'Wallets-2', frame: '8.' } },
  {
    name: 'wallet-exchange-sub',
    route: '/stores',
    steps: [{ eval: toWallet('Bybit') }, { click: '[data-tour="wallet-gear"]' }, { wait: 300 }, { click: '[data-gear="sub-account"]' }, { wait: 500 }],
    canvas: { board: 'Wallets-2', frame: '9.' },
  },
  { name: 'wallet-property', route: '/stores', steps: [{ eval: toWallet('Квартира') }], canvas: { board: 'Wallets-2', frame: '10.' } },
  { name: 'wallet-utility-zones', route: '/stores', steps: [{ eval: toWallet('Квартира') }, { eval: openLine('Электр') }], canvas: { board: 'Wallets-2', frame: '11.' } },
  { name: 'wallet-utility-formula', route: '/stores', steps: [{ eval: toWallet('Квартира') }, { eval: openLine('Горяч') }], canvas: { board: 'Wallets-2', frame: '12.' } },
  {
    name: 'wallet-utility-bill',
    route: '/stores',
    steps: [
      { eval: toWallet('Квартира') },
      { eval: PAY_BILL },
      { fill: ['[aria-label="Электричество: день"]', '13520'] },
      { fill: ['[aria-label="Электричество: ночь"]', '5190'] },
      { wait: 900 },
    ],
    canvas: { board: 'Wallets-2', frame: '13.' },
  },
  { name: 'utility-new-kind', route: '/stores', steps: [{ eval: toWallet('Квартира') }, { eval: NEW_ITEM }], canvas: { board: 'Wallets-3', frame: '14.' } },
  {
    name: 'utility-new-meter',
    route: '/stores',
    steps: [...itemStep2('Холодная вода'), { fill: ['#utility-tariff', '49'] }, { wait: 900 }],
    canvas: { board: 'Wallets-3', frame: '15.' },
  },
  {
    name: 'utility-new-formula',
    route: '/stores',
    steps: [
      ...itemStep2('Холодная вода'),
      { click: '[data-utility-mode] button:nth-child(2)' },
      { fill: ['#utility-formula', '(показание - предыдущее) * 3,5 * 10'] },
      { wait: 1200 },
    ],
    canvas: { board: 'Wallets-3', frame: '16.' },
  },
  {
    name: 'utility-new-rent',
    route: '/stores',
    steps: [...itemStep2('Аренда', 3), { fill: ['#utility-amount', '35000'] }, { fill: ['#utility-due', '10'] }, { wait: 500 }],
    canvas: { board: 'Wallets-3', frame: '17.' },
  },
  {
    // a new vault: the «Наличные» it comes with, nothing in it
    name: 'wallet-cash-empty',
    route: '/stores',
    scenarios: ['fresh'],
    steps: [{ eval: toWallet('Наличные') }],
    canvas: { board: 'Wallets-3', frame: '18.' },
  },
  { name: 'wallet-settings', route: '/stores', steps: [{ eval: toWallet('Bybit') }, ...gear('settings')], canvas: { board: 'Wallets-3', frame: '19.' } },
  {
    name: 'wallet-adjust',
    route: '/stores',
    // one currency: the short form (Тинькофф holds RUB and USD in the mock, so it gets the table)
    steps: [{ eval: toWallet('Наличные') }, ...gear('adjust'), { fill: ['#adjustment-actual', '95400'] }, { wait: 400 }],
    canvas: { board: 'Wallets-3', frame: '20.' },
  },
  {
    name: 'wallet-cash-balance',
    route: '/stores',
    scenarios: ['fresh'],
    steps: [{ eval: toWallet('Наличные') }, { click: '[data-wallet-balance]' }, { wait: 600 }],
    canvas: { board: 'Wallets-3', frame: '20.' },
  },
  {
    name: 'property-sell',
    route: '/stores',
    steps: [
      { eval: toWallet('Машина') },
      { desktop: [{ click: '[data-wallet-sell]' }] },
      { phone: [{ click: '[data-tour="wallet-gear"]' }, { wait: 300 }, { click: '[data-gear="sell"]' }] },
      { wait: 500 },
      { fill: ['#sell-received', '12000'] }, { wait: 400 }],
    canvas: { board: 'Wallets-4', frame: '21.' },
  },
  { name: 'currency-settings', route: 'settings:accounting', steps: [{ wait: 800 }], canvas: { board: 'Currency-1', frame: '1.' } },
  {
    name: 'money-card',
    route: '/',
    steps: [
      { wait: 1200 },
      {
        eval:
          "const s = [...document.querySelectorAll('main [data-money-currency=rub]')].find((n) => n.getClientRects().length && !n.parentElement.closest('a, button')); const m = document.querySelector('main'); if (m.scrollWidth > m.clientWidth + 1) throw new Error('main overflows: ' + m.scrollWidth + ' > ' + m.clientWidth); s.scrollIntoView({ block: 'center' }); s.click(); return !!s",
      },
      { wait: 900 },
    ],
    canvas: { board: 'Currency-1', frame: '5.' },
  },
  {
    name: 'convert-overpay',
    route: '/',
    steps: [
      { eval: toWallet('Тинькофф') },
      { click: { text: 'Обменять', within: 'main' } },
      { wait: 900 },
      { fill: ['#tx-amount', '46500'] },
      { fill: ['#tx-to-amount', '480'] },
      { eval: "[...document.querySelectorAll('[data-asset-chip]')].filter((n) => n.getClientRects().length)[1].click(); return true" },
      { wait: 500 },
      { eval: "[...document.querySelectorAll('[data-slot=combobox-item]')].find((n) => /\\bUSDT\\b/.test(n.textContent)).click(); return true" },
      { wait: 400 },
      { click: { text: '— Выберите —', within: '[role=dialog]' } },
      { wait: 500 },
      { click: { text: 'Bybit', within: '[role=listbox]' } },
      { wait: 1200 },
    ],
    canvas: { board: 'Currency-1', frame: '6.' },
  },
  {
    name: 'spending-unpriced',
    route: '/money',
    scenarios: ['stress'],
    steps: [{ wait: 1500 }],
    canvas: { board: 'Currency-1', frame: '7.' },
  },
  {
    name: 'rates-offline',
    route: '/',
    scenarios: ['stress'],
    steps: [{ eval: OFFLINE }, { wait: 1500 }],
    canvas: { board: 'Currency-2', frame: '8.' },
  },
  { name: 'rates-stale', route: '/', scenarios: ['stress'], steps: [{ eval: ONLINE }, { wait: 1500 }] },
  { name: 'rates-refreshed', route: '/', scenarios: ['stress'], steps: [{ eval: ONLINE }, { wait: 1500 }, { click: '[data-rates-refresh]' }, { wait: 1500 }] },
  {
    name: 'spending-unpriced-show',
    route: '/money',
    scenarios: ['stress'],
    steps: [{ wait: 1500 }, { click: '[data-spending-incomplete-show]' }, { wait: 1500 }],
  },
  {
    name: 'base-change',
    route: 'settings:accounting',
    steps: [
      { wait: 800 },
      { click: '#base-currency' },
      { wait: 400 },
      { type: ['input:focus', 'rub'] },
      { wait: 400 },
      { press: 'Enter' },
      { wait: 800 },
    ],
    canvas: { board: 'Currency-2', frame: '9.' },
  },
  {
    name: 'palette-convert',
    route: '/',
    steps: [{ wait: 600 }, ...PALETTE, { wait: 300 }, { type: ['[data-command-palette] input', '100 usd'] }, { wait: 1200 }],
    canvas: { board: 'Currency-2', frame: '10.' },
  },
  {
    name: 'palette-convert-dated',
    route: '/',
    steps: [{ wait: 600 }, ...PALETTE, { wait: 300 }, { type: ['[data-command-palette] input', '5000 руб'] }, { wait: 1500 }],
    canvas: { board: 'Currency-2', frame: '10.' },
  },
  {
    name: 'palette-convert-to',
    route: '/',
    steps: [{ wait: 600 }, ...PALETTE, { wait: 300 }, { type: ['[data-command-palette] input', '0.01 btc в thb'] }, { wait: 1200 }],
    canvas: { board: 'Currency-2', frame: '10.' },
  },
  { name: 'wallet-bank-currency', route: '/stores', steps: [{ eval: toWallet('Тинькофф') }], canvas: { board: 'Currency-1', frame: '3.' } },
  { name: 'wallet-exchange-currency', route: '/stores', steps: [{ eval: toWallet('Bybit') }], canvas: { board: 'Currency-1', frame: '4.' } },
  { name: 'store-new-kind', route: '/stores', steps: CREATE_STORE, canvas: { board: 'Wallets-1', frame: '2.' } },
  {
    name: 'store-new-name',
    route: '/stores',
    steps: [...CREATE_STORE, { click: '[data-wizard-next]' }, { wait: 400 }, { fill: ['#store-name', 'Тинькофф'] }],
    canvas: { board: 'Wallets-1', frame: '3.' },
  },
  {
    name: 'store-new-opening',
    route: '/stores',
    steps: [
      ...CREATE_STORE,
      { click: '[data-wizard-next]' },
      { wait: 400 },
      { fill: ['#store-name', 'Тинькофф'] },
      { click: '[data-wizard-next]' },
      { wait: 400 },
      { fill: ['#store-opening-amount', '84320'] },
    ],
    canvas: { board: 'Wallets-1', frame: '4.' },
  },
]

export const SETS = { before, stage1, stage2, stage2b, paths, stage3, stage4, stage5, stage6, links, money, moneyScreens, encrypt, samecur }
