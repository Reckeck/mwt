import { GP } from './steps.mjs'

/** Stage 6½ · Links (docs/ui-links.md): ways in and out, not on the canvas. */

const wait = `const sleep = (ms) => new Promise((done) => setTimeout(done, ms))`

/** The first wallet of the list, opened by its page (a root wallet: `Wallets ›` above it, «‹» on a phone). */
const FIRST_WALLET = `
  ${GP} ${wait}
  await gp.$router.push('/stores'); await sleep(700)
  const link = document.querySelector('a[href^="/stores/"]')
  if (link) await gp.$router.push(link.getAttribute('href'))
  await sleep(900); return true`

/** Two cards stacked: a contact under a goal; the goal's sheet shows `← Back`. */
const CARD_STACK = `
  ${GP} ${wait}
  const { call } = await import('/src/shared/api/ipc.ts')
  const goal = (await call('list_goals'))[0]
  const contact = (await call('list_contacts'))[0]
  await gp.$router.push({ path: '/money/plan', query: { open: 'contact:' + contact.id + '~goal:' + goal.id } })
  await sleep(1000); return true`

/** A row pointed at by `?focus=`: the list scrolls to it and lights it (a goal on the plan, a reminder). */
const focusOn = (command, path, area) => `
  ${GP} ${wait}
  const { call } = await import('/src/shared/api/ipc.ts')
  const rows = await call('${command}')
  await gp.$router.push('${path}'); await sleep(600)
  await gp.$router.push({ path: '${path}', query: { focus: rows.at(-1).${area} } })
  await sleep(1100); return true`

export const links = [
  { name: 'link-wallet-root', route: '/', steps: [{ eval: FIRST_WALLET }] },
  { name: 'link-home-chip', route: '/', steps: [{ click: { text: 'Все операции' } }, { wait: 900 }] },
  { name: 'link-card-stack', route: '/', steps: [{ eval: CARD_STACK }] },
  { name: 'link-focus-goal', route: '/', steps: [{ eval: focusOn('list_goals', '/money/plan', 'id') }] },
  { name: 'link-focus-reminder', route: '/', steps: [{ eval: focusOn('list_reminders', '/automations/reminders', 'id') }] },
  { name: 'link-glance-capital', route: '/', steps: [{ click: '[data-glance-trigger="capital"]' }, { wait: 600 }] },
  { name: 'link-money-card', route: '/transactions', steps: [{ click: '[data-money-convertible]' }, { wait: 600 }] },
]
