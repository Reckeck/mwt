import { encrypt } from './sets-encrypt.mjs'
import { login } from './sets-recents.mjs'
import { sandbox } from './sets-sandbox.mjs'
import { stage5 } from './sets-stage5.mjs'
import { stage6 } from './sets-stage6.mjs'
import { GP } from './steps.mjs'

/** X3 fixes: automations, settings, wallets, currencies, unlock, home, palette. */
const named = (list, name, extra = {}) => ({ ...list.find((shot) => shot.name === name), ...extra, canvas: undefined })
const at = (group, section) => `/?settings=${group}${section ? `&section=${section}` : ''}`
const PALETTE = [{ desktop: [{ press: 'ctrl+k' }] }, { phone: [{ click: '[data-command-palette-trigger]' }] }, { wait: 600 }]
const toBybit = `
  ${GP}
  await gp.$router.push('/stores'); await new Promise((done) => setTimeout(done, 700))
  const link = [...document.querySelectorAll('a[href^="/stores/"]')].find((a) => a.textContent.includes('Bybit'))
  if (link) await gp.$router.push(link.getAttribute('href'))
  await new Promise((done) => setTimeout(done, 900)); return true`
const EDIT_SCHEME = named(stage5, 'auto-scheme-error').steps

export const fixx3 = [
  { name: 'fixX3-accounting', route: at('accounting', ''), steps: [{ wait: 800 }] },
  { name: 'fixX3-sync', route: at('sync', ''), steps: [{ wait: 800 }] },
  { name: 'fixX3-keys', route: at('app', 'quick-add'), steps: [{ wait: 800 }] },
  { name: 'fixX3-fx', route: at('accounting', ''), steps: [{ wait: 800 }, { eval: `document.querySelector('table')?.scrollIntoView({ block: 'start' }); return true` }, { wait: 400 }] },
  { name: 'fixX3-app', route: at('app', ''), steps: [{ wait: 800 }] },
  { name: 'fixX3-palette', route: '/', steps: PALETTE },
  { name: 'fixX3-palette-convert', route: '/', steps: [...PALETTE, { type: ['input', '100 usd'] }, { wait: 800 }] },
  { name: 'fixX3-journal', route: '/automations/journal', steps: [{ wait: 800 }] },
  { name: 'fixX3-salary', route: '/automations?new=salary', steps: [{ wait: 700 }] },
  {
    name: 'fixX3-import',
    route: '/automations/workflows',
    steps: [...named(stage5, 'auto-import').steps, { click: '[form="workflow-import"]' }, { wait: 500 }],
  },
  {
    name: 'fixX3-exchange-sub',
    route: '/stores',
    steps: [{ eval: toBybit }, { click: '[data-tour="wallet-gear"]' }, { wait: 300 }, { click: '[data-gear="sub-account"]' }, { wait: 600 }],
  },
  {
    name: 'fixX3-confirm-category',
    route: at('accounting', 'categories'),
    steps: [{ wait: 800 }, { click: '[data-category-row] [data-slot="confirm-button"]' }, { wait: 600 }],
  },
  {
    name: 'fixX3-confirm-sheet',
    route: '/automations',
    steps: [{ click: '[data-mine-row^="scheme"] a' }, { wait: 600 }, { click: '[data-automation-delete]' }, { wait: 500 }],
  },
  named(encrypt, 'enc-set', { name: 'fixX3-set-password' }),
  named(encrypt, 'enc-remove', { name: 'fixX3-remove-password' }),
  { name: 'fixX3-password-change', ...named(stage6, 'set-vault-password') },
  named(login, 'login-missing', { name: 'fixX3-login-missing' }),
  { name: 'fixX3-home-error', route: '/', scenarios: ['error'], steps: [{ wait: 9000 }] },
  { name: 'fixX3-unsaved', route: '/automations', steps: [...EDIT_SCHEME, { eval: `${GP} gp.$router.push('/stores'); return true` }, { wait: 800 }] },
]

/** UX-2 · 1: touch targets of 40px on a phone (buttons, chips, switch rows, the sheet's «×»). */
fixx3.push(
  { name: 'touch-home', route: '/', steps: [{ wait: 900 }] },
  { name: 'touch-ops-more', route: '/transactions', steps: [{ wait: 600 }, { click: '[data-filter-more]' }, { wait: 500 }] },
  { name: 'touch-auto-mine', route: '/automations', steps: [{ wait: 800 }] },
  named(sandbox, 'sbx-bar', { name: 'touch-sbx-bar' }),
  named(sandbox, 'sbx-reset', { name: 'touch-sbx-reset' }),
  named(login, 'login-delete', { name: 'touch-login-delete' }),
)
