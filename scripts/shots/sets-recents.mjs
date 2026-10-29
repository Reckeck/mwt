/** `Sign in` on the unlock screen: Full-New-Recents 1–4 and Full-New-Encrypt 3. No login, each shot prepares its own mock state. */

const FIELD = '[data-recents-field]'
/** A returning user: the base currency is chosen, so the card does not ask for it. */
const chosen = {
  eval: `
    const app = await import('/src/shared/api/mock/vault-data.ts')
    app.mockAppSettings.base_currency_chosen = true
    for (let tries = 0; !document.querySelector('#app')?.__vue_app__ && tries < 300; tries++) await new Promise((done) => setTimeout(done, 100))
    await document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('settings').loadApp()
    return true`,
}
const open = [chosen, { click: FIELD }, { wait: 400 }]

/** Page code: the mock's history rows and the vault store, to bend them before the shot. */
const MOCK = `
  const files = await import('/src/shared/api/mock/commands-vault-files.ts')
  const data = await import('/src/shared/api/mock/vault-data.ts')
  for (let tries = 0; !document.querySelector('#app')?.__vue_app__ && tries < 300; tries++) await new Promise((done) => setTimeout(done, 100))
  const store = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('vault')`

/** Pick the n-th history row (0-based) in the open list. */
const pickRow = (index) => ({
  eval: `document.querySelectorAll('[data-recents-row] > button:first-child')[${index}].click(); return true`,
})

export const login = [
  { name: 'login-recents', route: '/unlock', login: false, steps: open, canvas: { board: 'New-Recents', frame: '1.' } },
  {
    name: 'login-forgotten',
    route: '/unlock',
    login: false,
    // The second row is not the one in the field; its × takes it out of the list.
    steps: [...open, { eval: `document.querySelectorAll('[data-recents-forget]')[1].click(); return true` }, { wait: 500 }],
    canvas: { board: 'New-Recents', frame: '2.' },
  },
  {
    // The chosen row has no file: the error under the field, `Find file…` and `Remove from history`.
    name: 'login-missing',
    route: '/unlock',
    login: false,
    steps: [...open, pickRow(2), { wait: 500 }],
    canvas: { board: 'New-Recents', frame: '3.' },
  },
  {
    // The app's own storage: a bin instead of «×», `Delete vault` for the second vault.
    name: 'login-delete',
    route: '/unlock',
    login: false,
    steps: [
      {
        eval: `${MOCK}
          files.mockRecents.splice(2, 1)
          data.mockStatus.value = { ...data.mockStatus.value, storage: 'app' }
          await store.refreshStatus()
          await store.loadRecents()
          return true`,
      },
      ...open,
      { eval: `document.querySelectorAll('[data-recents-delete]')[1].click(); return true` },
      { wait: 600 },
    ],
    canvas: { board: 'New-Recents', frame: '4.' },
  },
  {
    // A vault without a password (core #85): the `no password` label, `Open` opens it at once.
    name: 'login-plain',
    route: '/unlock',
    login: false,
    steps: [
      {
        eval: `${MOCK}
          files.mockRecents[1].encrypted = false
          await store.loadRecents()
          return true`,
      },
      ...open,
      pickRow(1),
      { wait: 500 },
    ],
    canvas: { board: 'New-Encrypt', frame: '3.' },
  },
]
