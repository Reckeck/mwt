/** Stage 6 · Settings (Settings-1…3): groups, tabs inside a group, the phone list of groups. */

const at = (group, section) => `/?settings=${group}${section ? `&section=${section}` : ''}`
const shot = (name, group, section, board, frame) => ({ name, route: at(group, section), steps: [{ wait: 600 }], canvas: { board, frame } })

export const stage6 = [
  shot('set-app', 'app', '', 'Settings-1', '1.'),
  shot('set-keys', 'app', 'quick-add', 'Settings-1', '2.'),
  shot('set-accounting', 'accounting', '', 'Settings-1', '3.'),
  shot('set-categories', 'accounting', 'categories', 'Settings-1', '4.'),
  shot('set-vault', 'vault', '', 'Settings-1', '5.'),
  { ...shot('set-vault-password', 'vault', '', 'Settings-1', '6.'), steps: [{ wait: 600 }, { click: { text: 'Сменить пароль' } }, { wait: 600 }] },
  // Not on the canvas: the restore question lists the copies and says what it replaces.
  { name: 'set-vault-restore', route: at('vault', ''), steps: [{ wait: 600 }, { click: { text: 'Восстановить…' } }, { wait: 600 }] },
  shot('set-sync', 'sync', '', 'Settings-1', '7.'),
  shot('set-devices', 'sync', 'sync-devices', 'Settings-2', '8.'),
  shot('set-history', 'sync', 'sync-history', 'Settings-2', '10.'),
  shot('set-transfer', 'sync', 'sync-transfer', 'Settings-2', '11.'),
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
