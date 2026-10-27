// Sub-accounts as tiles under a wallet's header and as unfoldable rows of the wallets table.
import { GP } from './steps.mjs'

/** A wallet's own page by the name in the wallets list. */
const toWallet = (name) => `
  ${GP}
  localStorage.removeItem('wallets-expanded')
  await gp.$router.push('/stores'); await new Promise((done) => setTimeout(done, 700))
  const link = [...document.querySelectorAll('a[href^="/stores/"]')].find((a) => a.textContent.includes('${name}'))
  if (link) await gp.$router.push(link.getAttribute('href'))
  await new Promise((done) => setTimeout(done, 1200)); return true`
/** The wallets with every chevron unfolded. */
const UNFOLD = `
  for (const button of document.querySelectorAll('[data-wallet-expand]')) if (button.getAttribute('aria-expanded') === 'false') button.click()
  await new Promise((done) => setTimeout(done, 500)); return true`
const FOLD = `localStorage.removeItem('wallets-expanded'); return true`

export const subaccounts = [
  { name: 'tinkoff', route: '/stores', steps: [{ eval: toWallet('Тинькофф') }] },
  { name: 'bybit', route: '/stores', steps: [{ eval: toWallet('Bybit') }] },
  { name: 'table-folded', route: '/stores', steps: [{ eval: FOLD }, { wait: 600 }] },
  { name: 'table-open', route: '/stores', steps: [{ eval: UNFOLD }, { wait: 300 }] },
]
