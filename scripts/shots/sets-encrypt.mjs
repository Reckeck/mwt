/** «Хранилище без пароля»: Full-New-Encrypt 1, 2 (create), 4, 5 (Settings › Vault). Login, 3 lives in sets-recents.mjs. */

/** Page code: the mock's vault state and the app's store, to bend them before the shot. */
const MOCK = `
  const data = await import('/src/shared/api/mock/vault-data.ts')
  const files = await import('/src/shared/api/mock/commands-vault-files.ts')
  for (let tries = 0; !document.querySelector('#app')?.__vue_app__ && tries < 300; tries++) await new Promise((done) => setTimeout(done, 100))
  const gp = document.querySelector('#app').__vue_app__.config.globalProperties
  const store = gp.$pinia._s.get('vault')`

const create = [{ click: '[data-unlock-create]' }, { wait: 600 }]
const typed = 'wallet-pass-1'

/** Settings › Vault with the mock bent by `bend`; another group first, so the link query is asked again. */
const settings = (bend, then) => [
  {
    eval: `${MOCK}
      ${bend}
      await store.refreshStatus()
      await gp.$router.push({ path: '/', query: { settings: 'app' } })
      await new Promise((done) => setTimeout(done, 500))
      await gp.$router.push({ path: '/', query: { settings: 'vault' } })
      return true`,
  },
  { wait: 900 },
  ...then,
  { wait: 600 },
]

export const encrypt = [
  {
    name: 'enc-create',
    route: '/unlock',
    login: false,
    steps: [...create, { fill: ['#create-password-new', typed] }, { fill: ['#create-password-repeat', typed] }, { wait: 300 }],
    canvas: { board: 'New-Encrypt', frame: '1.' },
  },
  {
    name: 'enc-create-plain',
    route: '/unlock',
    login: false,
    steps: [...create, { click: { text: 'Без пароля' } }, { wait: 400 }],
    canvas: { board: 'New-Encrypt', frame: '2.' },
  },
  {
    // The password question: the vault has an external copy, so the consent for it is asked too.
    name: 'enc-remove',
    route: '/',
    steps: settings(
      `files.mockLink.value = { linked: true, display_name: 'Download/vault.wallet', last_synced_at: '2026-10-22T09:40:00Z', last_error: null }`,
      [{ click: { text: 'Отключить пароль' } }, { fill: ['#remove-password-current', 'wallet-pass'] }],
    ),
    canvas: { board: 'New-Encrypt', frame: '4.' },
  },
  {
    // No password: the label, «Задать пароль…» and its question.
    name: 'enc-set',
    route: '/',
    steps: settings(
      `data.mockPassword.value = null
      data.mockInfo.encrypted = false
      data.mockStatus.value = { ...data.mockStatus.value, encrypted: false }`,
      [
        { click: { text: 'Задать пароль' } },
        { fill: ['#set-password-new', typed] },
        { fill: ['#set-password-repeat', typed] },
      ],
    ),
    canvas: { board: 'New-Encrypt', frame: '5.' },
  },
]
