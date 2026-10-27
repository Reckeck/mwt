// «Что изменилось» on Home: the day band under the title, a reconciliation row, the day total.
import { GP } from './steps.mjs'

/** Home last seen three days ago; a reconciliation of the Bybit wallet booked yesterday. */
const SEED = `
  ${GP}
  const wait = (ms) => new Promise((done) => setTimeout(done, ms))
  const day = (back) => { const d = new Date(Date.now() - back * 86400000); return d.toISOString().slice(0, 10) }
  localStorage.setItem('trade-app:home-visit', JSON.stringify({ seen: day(0), since: day(3) }))
  const { call } = await import('/src/shared/api/ipc.ts')
  const stores = await call('list_stores')
  const store = stores.find((s) => s.name.includes('Bybit')) ?? stores[0]
  const at = new Date(Date.now() - 86400000).toISOString()
  await call('add_transaction', { input: { type: 'adjustment', common: { timestamp: at, comment: 'Сверка с биржей: живой баланс', categories: [], flow_id: null, contact_id: null, price_usd: null, property_id: null }, store_id: store.id, lines: [{ asset: { symbol: 'USDT', name: 'Tether', coingecko_id: 'tether', chain: null, contract: null }, actual: '0.1834' }] } })
  await gp.$router.push('/transactions'); await wait(600)
  await gp.$router.push('/'); await wait(1200)
  return true`
const SCROLL = `document.querySelector('[data-tour="home-changes"]')?.scrollIntoView({ block: 'center' }); await new Promise((done) => setTimeout(done, 300)); return true`

export const homeChanges = [
  { name: 'home-changes', route: '/', steps: [{ eval: SEED }, { eval: SCROLL }] },
  { name: 'left-today-card', route: '/', steps: [{ wait: 800 }, { desktop: [{ click: '.value.big [data-money-convertible]' }] }, { phone: [{ tap: '.value.big [data-money-convertible]' }] }, { wait: 800 }] },
  { name: 'settings-accounting', route: '/?settings=accounting&section=currencies', steps: [{ wait: 800 }] },
  { name: 'settings-list', route: '/?settings=list', steps: [{ wait: 800 }], desktop: false },
  { name: 'ops', route: '/transactions', steps: [{ wait: 800 }] },
]
