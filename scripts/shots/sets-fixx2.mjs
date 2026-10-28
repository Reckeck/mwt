import { GP } from './steps.mjs'

const TO_BYBIT = `
  ${GP}
  await gp.$router.push('/stores'); await new Promise((done) => setTimeout(done, 700))
  const link = [...document.querySelectorAll('a[href^="/stores/"]')].find((a) => a.textContent.includes('Bybit'))
  if (link) await gp.$router.push(link.getAttribute('href'))
  await new Promise((done) => setTimeout(done, 900)); return true`


/** Pack 2 · Операции и Главная: both currencies THB on the form's own state (the chip's search clears it while typing). */
const P2_BOTH_THB = `
  const thb = { symbol: 'THB', name: 'THB', coingecko_id: null, chain: 'fiat', contract: null, quote: 'THB' }
  let node = document.querySelector('#transaction-form').__vueParentComponent
  let ctx = null
  while (node && !ctx) {
    const key = Object.getOwnPropertySymbols(node.provides ?? {}).find((symbol) => symbol.description === 'tx-form')
    ctx = key ? node.provides[key] : null
    node = node.parent
  }
  ctx.form.fromAsset = { ...thb }
  ctx.form.toAsset = { ...thb }
  return true`
const P2_SAME = [
  { wait: 600 },
  { desktop: [{ press: 'ctrl+n' }] },
  { phone: [{ eval: `const nav = document.querySelector('[data-tour="bottom-nav"]'); [...nav.querySelectorAll('button, a')].find((el) => !el.textContent.trim())?.click(); return true` }] },
  { wait: 800 },
  { click: '[data-kind-tab="convert"]' },
  { wait: 600 },
  { fill: ['#tx-amount', '3000'] },
  { fill: ['#tx-to-amount', '3000'] },
  { eval: P2_BOTH_THB },
  { wait: 400 },
  { click: { text: '— Выберите —', within: '[role=dialog]' } },
  { wait: 500 },
  { eval: "document.querySelector('[role=listbox] [role=option]:not([aria-disabled=true])').click(); return true" },
  { wait: 500 },
]
/** The list's filter outlives a route without a query: a filtered shot starts from a clean one. */
const P2_CLEAN = { eval: `const { sharedTxFilter } = await import('/src/pages/transactions/tx-filter.state.ts'); const { emptyTxFilter } = await import('/src/shared/lib/tx-filter.ts'); sharedTxFilter.value = emptyTxFilter(); return true` }

/** X2 fixes: toasts on a phone, filter line, tx card, salary dialog, row hold, deal, route labels, calendar. */
export const fixx2 = [
  {
    name: 'fixX2-salary-err',
    route: '/automations?new=salary',
    steps: [{ wait: 500 }, { click: '[data-salary-count="2"]' }, { click: { text: 'Создать', within: '[role="dialog"]' } }, { wait: 600 }],
    scenarios: ['year'],
  },
  {
    name: 'fixX2-hold',
    route: '/transactions',
    desktop: false,
    steps: [{ wait: 800 }, { hold: '[data-tx-row]' }, { wait: 600 }],
  },
  {
    name: 'fixX2-hold2',
    route: '/transactions',
    desktop: false,
    steps: [{ wait: 800 }, { eval: `const rows = [...document.querySelectorAll('[data-tx-row]')]; rows[rows.length > 6 ? 6 : rows.length - 1].setAttribute('data-x2', '1'); return true` }, { hold: '[data-x2]' }, { wait: 600 }],
  },
  {
    name: 'fixX2-toasts',
    route: '/',
    steps: [
      { wait: 800 },
      { desktop: [{ press: 'ctrl+n' }] },
      { phone: [{ eval: `const nav = document.querySelector('[data-tour="bottom-nav"]'); [...nav.querySelectorAll('button, a')].find((el) => !el.textContent.trim())?.click(); return true` }] },
      { wait: 900 },
      { eval: `const { notify } = await import('/src/shared/lib/notify/index.ts'); for (const text of ['Оплата 340 USDT · Аренда сохранена', 'Оплата 120 USDT · Аренда сохранена', 'Оплата 99 USDT · Аренда сохранена']) { notify.success(text, { description: 'Кошелёк, категория и дата остались', action: { label: 'Отменить', onClick: () => {} } }); await new Promise((done) => setTimeout(done, 250)) } return true` },
      { wait: 600 },
    ],
  },
  {
    name: 'fixX2-filter',
    route: '/transactions',
    steps: [{ wait: 800 }, { click: '[data-filter-more]' }, { wait: 500 }, { click: { text: '3 мес' } }, { wait: 500 }, { click: '[data-filter-apply]' }, { wait: 900 }],
    desktop: true,
    phone: false,
  },
  {
    name: 'fixX2-filter-off',
    route: '/transactions',
    steps: [{ wait: 1000 }],
    phone: false,
  },
  {
    name: 'fixX2-card',
    route: '/transactions',
    steps: [{ wait: 800 }, { eval: `document.querySelectorAll('[data-tx-open]')[1].click(); return true` }, { wait: 900 }, { click: '[data-tx-next]' }, { wait: 900 }],
    phone: false,
  },
  {
    name: 'fixX2-card-pay',
    route: '/transactions',
    steps: [{ wait: 800 }, { eval: `document.querySelectorAll('[data-tx-open]')[0].click(); return true` }, { wait: 900 }],
  },
  {
    name: 'fixX2-defi',
    route: '/',
    steps: [
      { eval: TO_BYBIT },
      { click: '[data-deal="defi"]' },
      { wait: 800 },
      { fill: ['#tx-amount', '99999'] },
      { wait: 500 },
      { click: { text: 'Добавить', within: '[role="dialog"]' } },
      { wait: 700 },
    ],
  },
  { name: 'fixX2-home', route: '/', steps: [{ wait: 1000 }] },
  { name: 'fixX2-bank', route: '/stores', steps: [{ wait: 600 }, { eval: `[...document.querySelectorAll('a[href^="/stores/"]')].find((a) => a.textContent.includes('Тинькофф'))?.click(); return true` }, { wait: 1200 }] },
  { name: 'ux2-list', route: '/transactions', steps: [{ wait: 1000 }] },
  {
    name: 'ux2-list-move',
    route: '/transactions',
    steps: [{ wait: 1000 }, { eval: `[...document.querySelectorAll('[data-tx-row]')].find((el) => el.textContent.includes('внутренний'))?.scrollIntoView({ block: 'center' }); return true` }, { wait: 400 }],
  },
  {
    name: 'ux2-card-move',
    route: '/transactions',
    steps: [{ wait: 900 }, { eval: `const row = [...document.querySelectorAll('[data-tx-row]')].find((el) => el.textContent.includes('внутренний')); row?.querySelector('[data-tx-open]')?.click(); return Boolean(row)` }, { wait: 900 }],
  },
  { name: 'fixX2-calendar', route: '/transactions', steps: [{ wait: 800 }, { click: { text: 'Календарь' } }, { wait: 1000 }], phone: false },
  // Pack 2 · Операции и Главная (filtered ones last: the list keeps its filter between shots).
  { name: 'p2-home', route: '/', steps: [P2_CLEAN, { wait: 1200 }] },
  { name: 'p2-same', route: '/', steps: P2_SAME },
  {
    name: 'p2-cashback',
    route: '/transactions',
    steps: [P2_CLEAN, { wait: 1000 }, { eval: `[...document.querySelectorAll('[data-tx-row]')].find((el) => el.textContent.includes('Кешбэк'))?.scrollIntoView({ block: 'center' }); return true` }, { wait: 400 }],
  },
  { name: 'p2-files', route: '/transactions?files=1', steps: [{ wait: 1200 }] },
  { name: 'p2-files-none', route: '/transactions?files=1&q=такси', steps: [{ wait: 1200 }] },
  { name: 'p2-files-reset', route: '/transactions?files=1&q=такси', steps: [{ wait: 1200 }, { click: '[data-filter-empty-reset]' }, { wait: 900 }] },
]
