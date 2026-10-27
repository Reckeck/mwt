/** Same-currency exchange: the hint under «Получаю», «Записать как перевод», the saved transfer. */

/** Both currencies set on the form's state itself (the chip's search clears the asset while typing): THB for «Отдаю» and «Получаю». */
const BOTH_THB = `
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

const OPEN = [
  { wait: 600 },
  { press: 'ctrl+n' },
  { wait: 700 },
  { click: '[data-kind-tab="convert"]' },
  { wait: 600 },
  { fill: ['#tx-amount', '3000'] },
  { fill: ['#tx-to-amount', '3000'] },
  { eval: BOTH_THB },
  { wait: 500 },
]
/** Into the second wallet: the picker opens on «— Выберите —». */
const TO_WALLET = [
  { click: { text: '— Выберите —', within: '[role=dialog]' } },
  { wait: 500 },
  { eval: "document.querySelector('[role=listbox] [role=option]:not([aria-disabled=true])').click(); return true" },
  { wait: 500 },
]

export const samecur = [
  { name: 'same-hint', route: '/', steps: OPEN },
  { name: 'same-submit', route: '/', steps: [...OPEN, ...TO_WALLET, { click: '[data-quick-add-submit]' }, { wait: 600 }] },
  { name: 'same-transfer', route: '/', steps: [...OPEN, ...TO_WALLET, { click: '[data-tx-as-transfer]' }, { wait: 600 }] },
  {
    name: 'same-transfer-short',
    route: '/',
    steps: [...OPEN, ...TO_WALLET, { fill: ['#tx-to-amount', '2990'] }, { click: '[data-tx-as-transfer]' }, { wait: 600 }],
  },
  {
    name: 'same-saved',
    route: '/',
    steps: [...OPEN, ...TO_WALLET, { click: '[data-tx-as-transfer]' }, { wait: 600 }, { click: '[data-quick-add-submit]' }, { wait: 900 }],
  },
]
