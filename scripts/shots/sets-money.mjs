/** Stage 6¾ · Money (docs/ui-money.md): a sum shows the other currencies on hover, a card on a tap or a hold. */

const PLAIN = '[data-money-convertible]:not(:is(a, button, [role="button"], [role="menuitem"]) *)'
const IN_BUTTON = ':is(a, button) [data-money-convertible]'

export const money = [
  { name: 'money-hover-text', route: '/', phone: false, steps: [{ hover: PLAIN }, { wait: 700 }] },
  { name: 'money-hover-dated', route: '/transactions', phone: false, steps: [{ hover: '[data-tx-row] [data-money-convertible]' }, { wait: 700 }] },
  { name: 'money-hover-button', route: '/', phone: false, steps: [{ hover: IN_BUTTON }, { wait: 700 }] },
  { name: 'money-tap-card', route: '/', desktop: false, steps: [{ tap: PLAIN }, { wait: 500 }] },
  { name: 'money-hold-button', route: '/stores', desktop: false, steps: [{ hold: IN_BUTTON }, { wait: 500 }] },
  { name: 'money-hold-row', route: '/transactions', desktop: false, steps: [{ hold: '[data-tx-row]' }, { wait: 500 }] },
]

/** Stage 6¾ visual check: the key screens where a sum is a `Money`, and a toast over a sheet on the phone. */
const PAUSE = 'await new Promise((done) => setTimeout(done, 900)); return true'
const GP = `
  for (let tries = 0; !document.querySelector('#app')?.__vue_app__ && tries < 300; tries++) await new Promise((done) => setTimeout(done, 100))
  const gp = document.querySelector('#app').__vue_app__.config.globalProperties;`
const toWallet = (name) => `
  ${GP}
  await gp.$router.push('/stores'); await new Promise((done) => setTimeout(done, 700))
  const link = [...document.querySelectorAll('a[href^="/stores/"]')].find((a) => a.textContent.includes('${name}'))
  if (link) await gp.$router.push(link.getAttribute('href'))
  ${PAUSE}`
const PAY_BILL = `
  ;[...document.querySelectorAll('[data-bill-pay]')].find((button) => button.offsetParent)?.click()
  ${PAUSE}`
const TOAST = `
  const { notify } = await import('/src/shared/lib/notify/index.ts')
  notify.success('Проверка: тост над шторкой', { action: { label: 'Отменить', onClick: () => {} } })
  await new Promise((done) => setTimeout(done, 600)); return true`

export const moneyScreens = [
  { name: 'mv-home', route: '/', steps: [{ wait: 800 }] },
  { name: 'mv-spending', route: '/money', steps: [{ wait: 800 }] },
  { name: 'mv-where', route: '/money/where', steps: [{ wait: 800 }] },
  { name: 'mv-plan', route: '/money/plan', steps: [{ wait: 800 }] },
  { name: 'mv-wallets', route: '/stores', steps: [{ wait: 800 }] },
  { name: 'mv-wallet', route: '/stores', steps: [{ eval: toWallet('Bybit') }] },
  { name: 'mv-transactions', route: '/transactions', steps: [{ wait: 800 }] },
  { name: 'mv-tx-card', route: '/transactions', steps: [{ wait: 800 }, { eval: `document.querySelector('[data-tx-open]').click(); return true` }, { wait: 900 }] },
  { name: 'mv-journal', route: '/automations/journal', steps: [{ wait: 800 }] },
  { name: 'mv-bill', route: '/stores', steps: [{ eval: toWallet('Квартира') }, { eval: PAY_BILL }, { wait: 600 }] },
  { name: 'mv-toast-sheet', route: '/money/plan', desktop: false, steps: [{ wait: 800 }, { click: '[data-entity^="goal:"]' }, { wait: 600 }, { eval: TOAST }] },
]
