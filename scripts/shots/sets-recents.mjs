/** «Недавние хранилища» on the unlock screen (Full-New-Recents 1. and 2.): no login, the screen as it is. */

const FIELD = '[data-recents-field]'
const open = [{ click: FIELD }, { wait: 400 }]

export const recents = [
  { name: 'recents-list', route: '/unlock', login: false, steps: open, canvas: { board: 'New-Recents', frame: '1.' } },
  {
    name: 'recents-forgot',
    route: '/unlock',
    login: false,
    // The second row is not the one in the field; its × takes it out of the list.
    steps: [...open, { eval: `document.querySelectorAll('[data-recents-forget]')[1].click(); return true` }, { wait: 500 }],
    canvas: { board: 'New-Recents', frame: '2.' },
  },
]
