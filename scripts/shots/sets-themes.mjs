// `mise run shots -- themes [--theme <id|all>]`: one theme on desktop and phone (TS2). Not a gate.
// Each shot waits for the screen to settle first; the theme itself comes from --theme (see shots.mjs).

const SETTLE = [{ wait: 1000 }]
/** The theme picker sits in the appearance block of the general settings. */
const PICKER = `
  const picker = document.querySelector('[data-theme-id]')
  picker?.scrollIntoView({ block: 'center' })
  await new Promise((done) => setTimeout(done, 400)); return true`
/** A transaction card is the dialog: it shows the amount, the wallet and the status in the theme's colours. */
const OPEN_DIALOG = `document.querySelector('[data-tx-open]')?.click(); await new Promise((done) => setTimeout(done, 900)); return true`

export const themes = [
  { name: 'home', route: '/', steps: SETTLE },
  // The operations list carries income, expense and transfer rows in the year scenario. The app has no in-transit
  // amount UI (spec: no new in-transit UI), so the in-transit colour is covered by the contrast pairs, not by a shot.
  { name: 'money', route: '/transactions', steps: SETTLE },
  { name: 'chart', route: '/money/where', steps: [{ wait: 1500 }] },
  { name: 'settings', route: 'settings:general', steps: [...SETTLE, { phone: [{ click: { text: 'Приложение' } }, { wait: 800 }] }, { eval: PICKER }] },
  { name: 'dialog', route: '/transactions', steps: [...SETTLE, { eval: OPEN_DIALOG }] },
]
