import { stage5 } from './sets-stage5.mjs'
import { GP } from './steps.mjs'

/** Pack 2c: automations and wallets — `Mine` header, emulation, wallet kind `Loyalty`, exchange import. */
const named = (name, extra = {}) => ({ ...stage5.find((shot) => shot.name === name), ...extra, canvas: undefined })
const toBybit = `
  ${GP}
  await gp.$router.push('/stores'); await new Promise((done) => setTimeout(done, 700))
  const link = [...document.querySelectorAll('a[href^="/stores/"]')].find((a) => a.textContent.includes('Bybit'))
  if (link) await gp.$router.push(link.getAttribute('href') + '?tab=import')
  await new Promise((done) => setTimeout(done, 900)); return true`
const NEW_STORE = [{ wait: 600 }, { click: '[data-tour="stores-create"]' }, { wait: 800 }]
/** `Loyalty and bonuses` and `Next`: the Bonuses wizard takes over. */
const TO_PROGRAM = [...NEW_STORE, { click: '[data-kind="loyalty"]' }, { wait: 400 }, { click: '[data-wizard-next]' }, { wait: 700 }]

export const pack2c = [
  { name: 'p2c-mine', route: '/automations', steps: [{ wait: 800 }] },
  named('auto-scheme-run', { name: 'p2c-emulation', phone: false }),
  { name: 'p2c-store-kind', route: '/stores', steps: NEW_STORE },
  { name: 'p2c-store-loyalty', route: '/stores', steps: TO_PROGRAM },
  {
    name: 'p2c-store-loyalty-made',
    route: '/stores',
    steps: [
      ...TO_PROGRAM,
      { fill: ['#program-name', 'Аэрофлот'] },
      { click: '#program-wallet' },
      { wait: 200 },
      { click: '[role="option"]' },
      { fill: ['#loyalty-value', '1'] },
      { click: '[data-wizard-next]' },
      { wait: 300 },
      { click: '[data-wizard-save]' },
      { wait: 900 },
      { eval: `${GP} await gp.$router.push('/transactions/bonuses'); await new Promise((done) => setTimeout(done, 900)); return true` },
    ],
  },
  { name: 'p2c-import', route: '/stores', steps: [{ eval: toBybit }] },
  {
    name: 'p2c-import-actions',
    route: '/stores',
    desktop: false,
    steps: [{ eval: toBybit }, { eval: `document.querySelector('[data-exchange-import]')?.scrollIntoView({ block: 'start' }); return true` }, { wait: 400 }],
  },
  { name: 'p2c-import-disconnect', route: '/stores', steps: [{ eval: toBybit }, { click: '[data-import-disconnect]' }, { wait: 500 }] },
]
