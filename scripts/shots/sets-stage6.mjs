import { encrypt } from './sets-encrypt.mjs'
import { login } from './sets-recents.mjs'
import { sandbox } from './sets-sandbox.mjs'

/** Stage 6 · Settings (Settings-1…3): groups, tabs inside a group, the phone list of groups. */

const at = (group, section) => `/?settings=${group}${section ? `&section=${section}` : ''}`
const shot = (name, group, section, board, frame) => ({ name, route: at(group, section), steps: [{ wait: 600 }], canvas: { board, frame } })

/** The devices shot comes after the sync shot of the same run: it relies on the phone paired here. */
/** The scripted phone of the mock: sync on, `Add a device`, the codes match (`Done`), the first changes applied. */
const pairPhone = [
  { wait: 600 },
  { click: { text: 'Синхронизировать по Wi-Fi' } },
  { wait: 900 },
  { click: { text: 'Устройства', within: '[role="tablist"]' } },
  { wait: 600 },
  { click: { text: 'Добавить устройство', within: '[data-section="sync-devices"]' } },
  { wait: 600 },
  { click: { text: 'Готово' } },
  { wait: 600 },
  { click: { text: 'Применить' } },
  { wait: 900 },
  { click: { text: 'Общее', within: '[role="tablist"]' } },
  { wait: 600 },
  { click: { text: 'Открыть порт' } },
  { wait: 6000 },
]

export const stage6 = [
  shot('set-app', 'app', '', 'Settings-1', '1.'),
  shot('set-keys', 'app', 'quick-add', 'Settings-1', '2.'),
  shot('set-accounting', 'accounting', '', 'Settings-1', '3.'),
  { name: 'set-money', route: at('accounting', 'budget'), steps: [{ wait: 600 }] },
  shot('set-categories', 'accounting', 'categories', 'Settings-1', '4.'),
  shot('set-vault', 'vault', '', 'Settings-1', '5.'),
  { ...shot('set-vault-password', 'vault', '', 'Settings-1', '6.'), steps: [{ wait: 600 }, { click: { text: 'Сменить пароль' } }, { wait: 600 }] },
  // Not on the canvas: the restore question lists the copies and says what it replaces.
  { name: 'set-vault-restore', route: at('vault', ''), steps: [{ wait: 600 }, { click: { text: 'Восстановить…' } }, { wait: 600 }] },
  { ...shot('set-sync', 'sync', '', 'Settings-1', '7.'), steps: [...pairPhone, { wait: 600 }] },
  { ...shot('set-devices', 'sync', 'sync-devices', 'Settings-2', '8.'), steps: [{ wait: 600 }] },
  // `Add a device` with the scripted phone nearby: the code, the time left, the phone found.
  // Comes after the devices shot: forgets the phone paired there and turns sync off and on to see it again.
  {
    ...shot('set-add-device', 'sync', 'sync-devices', 'Settings-2', '9.'),
    steps: [
      { wait: 600 },
      { click: { text: 'Забыть', within: '[data-section="sync-devices"]' } },
      { click: { text: 'Забыть', within: '[data-section="sync-devices"]' } },
      { wait: 600 },
      { click: { text: 'Общее', within: '[role="tablist"]' } },
      { wait: 600 },
      { click: { text: 'Синхронизировать по Wi-Fi' } },
      { wait: 600 },
      { click: { text: 'Синхронизировать по Wi-Fi' } },
      { wait: 900 },
      { click: { text: 'Устройства', within: '[role="tablist"]' } },
      { wait: 600 },
      { click: { text: 'Добавить устройство', within: '[data-section="sync-devices"]' } },
      { wait: 800 },
    ],
  },
  // Pair the phone (its changes wait for an answer), then open `History`: the block with `Accept` above the journal.
  {
    ...shot('set-sync-history', 'sync', 'sync-history', 'Settings-2', '10.'),
    steps: [
      // Whatever the shots before left: start over with the mock's own commands, the phone paired, its changes waiting.
      {
        eval: `
          const { call } = await import('/src/shared/api/ipc.ts')
          const off = { enabled: false, confirm_first: null, prefer_main: null, device_name: null, background_lock_min: null }
          await call('sync_forget_peer', { deviceId: 'mock-phone' })
          await call('sync_set_settings', { input: off })
          await call('sync_set_settings', { input: { ...off, enabled: true } })
          await call('sync_pair', { sessionId: 'mock-session' })
          await call('sync_confirm_pairing', { input: { session_id: 'mock-session', accept: true } })
          const gp = document.querySelector('#app').__vue_app__.config.globalProperties
          await gp.$router.push({ path: '/', query: { settings: 'sync', section: 'sync-history' } })
          return true`,
      },
      { wait: 900 },
    ],
  },
  shot('set-transfer', 'sync', 'sync-transfer', 'Settings-2', '11.'),
  // `Receive`: this device waits under its name, the time left, the way in the menu of the other one (Settings-2 · 12).
  {
    ...shot('set-receive', 'sync', 'sync-transfer', 'Settings-2', '12.'),
    steps: [{ wait: 600 }, { click: { text: 'Получить', within: '[data-section="sync-transfer"]' } }, { wait: 700 }],
  },
  shot('set-help', 'help', '', 'Settings-2', '13.'),
  shot('set-dev', 'dev', '', 'Settings-2', '14.'),
]

stage6.push({ name: 'set-list', route: at('list', ''), steps: [{ wait: 600 }], desktop: false, canvas: { board: 'Settings-3', frame: '15.' } })

/** Phone: tap a group in the list opens its screen; «‹» there goes back to the list. */
const tap = { click: { text: 'Учёт', within: '[data-tour="settings-shell"]' } }
const back = { click: '[data-tour="settings-shell"] [aria-label="Назад"]' }
stage6.push(
  { name: 'set-list-open', route: at('list', ''), steps: [tap, { wait: 600 }], desktop: false },
  { name: 'set-list-back', route: at('list', ''), steps: [tap, { wait: 600 }, back, { wait: 600 }], desktop: false },
)

/** Unlock screen: recents, forgotten, file not found, delete in app storage, no password. */
stage6.push(...login)

/** Vault without a password: create, remove, set. After the rest: they bend the mock's vault and its external copy. */
stage6.push(...encrypt)

/** Sandbox: the entrance, the bar, `Reset the sandbox?`, after `Exit`. */
stage6.push(...sandbox)
