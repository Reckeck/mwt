/** `Sandbox`: Full-New-Sandbox 1–4. No login: each shot starts on the unlock screen and goes in through the real button. */

/** A returning user: the base currency is chosen, so the card does not ask for it. */
const chosen = {
  eval: `
    const app = await import('/src/shared/api/mock/vault-data.ts')
    app.mockAppSettings.base_currency_chosen = true
    // A year of `What's new` is not what these shots show.
    app.mockAppSettings.installed_at = '2999-01-01'
    for (let tries = 0; !document.querySelector('#app')?.__vue_app__ && tries < 300; tries++) await new Promise((done) => setTimeout(done, 100))
    await document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('settings').loadApp()
    return true`,
}
/** `Open the sandbox` on the card; the dashboard follows. */
const enter = [
  chosen,
  { click: '[data-sandbox-open]' },
  // The dashboard's tour starts after a second and covers the page: close it before the next click.
  { wait: 1800 },
  { eval: `document.querySelector('.driver-popover-close-btn')?.click(); return true` },
  { wait: 300 },
]
/** The page code: the router, to open another screen once inside. */
const goto = (path) => ({
  eval: `const gp = document.querySelector('#app').__vue_app__.config.globalProperties; await gp.$router.push('${path}'); return true`,
})

export const sandbox = [
  { name: 'sbx-login', route: '/unlock', login: false, steps: [chosen, { wait: 400 }], canvas: { board: 'New-Sandbox', frame: '1.' } },
  // Home, as the task asks; the canvas draws the bar over Operations, the next shot is that screen.
  { name: 'sbx-bar', route: '/unlock', login: false, steps: [...enter, { wait: 500 }], canvas: { board: 'New-Sandbox', frame: '2.' } },
  { name: 'sbx-bar-ops', route: '/unlock', login: false, steps: [...enter, goto('/transactions'), { wait: 900 }] },
  {
    name: 'sbx-reset',
    route: '/unlock',
    login: false,
    steps: [...enter, goto('/transactions'), { wait: 900 }, { click: '[data-sandbox-reset]' }, { wait: 600 }],
    canvas: { board: 'New-Sandbox', frame: '3.' },
  },
  {
    name: 'sbx-after',
    route: '/unlock',
    login: false,
    steps: [...enter, { click: '[data-sandbox-exit]' }, { wait: 1200 }],
    canvas: { board: 'New-Sandbox', frame: '4.' },
  },
]
