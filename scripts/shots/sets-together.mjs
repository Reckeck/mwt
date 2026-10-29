import { GP } from './steps.mjs'

/** «Вместе» (shared events): the first event, plan and purchases, how to split. `mise run shots -- together`. */

const EVENT = '/money/together/mock-event-skewers'
/** The list and the event are TanStack queries: calls made straight through `call` leave them stale. */
const REFRESH = `
  const provides = document.querySelector('#app').__vue_app__._context.provides
  for (const key of [...Object.getOwnPropertySymbols(provides), ...Object.keys(provides)]) {
    if (typeof provides[key]?.invalidateQueries === 'function') await provides[key].invalidateQueries()
  }`
const wait = `const sleep = (ms) => new Promise((done) => setTimeout(done, ms))`

/** The picker inside the new-event dialog takes real key input; then the row of the suggestion is clicked. */
const PEOPLE_INPUT = '[data-tour="shared-edit-people"] input'
const ROW_MENU = (row) => `tbody tr:nth-child(${row}) [aria-label="Ещё"]`
/** A real click on a computer, a tap on a phone (the shot runner moves a finger there). */
const press = (css) => [{ desktop: [{ click: css }] }, { phone: [{ tap: css }] }]
const pressText = (text) => [{ desktop: [{ click: { text } }] }, { phone: [{ tap: { text } }] }]

/** The new-event dialog with a name, two days and three people typed in. */
const FILL_EVENT = [
  ...pressText('Новое событие'),
  { wait: 500 },
  { fill: ['#event-title', 'Шашлыки'] },
  { type: [PEOPLE_INPUT, 'Оля'] },
  { wait: 300 },
  ...pressText('Добавить гостем'),
  { wait: 300 },
  { type: [PEOPLE_INPUT, 'Пет'] },
  { wait: 300 },
  ...pressText('Петя'),
  { wait: 300 },
]

/** A fresh planned event with five people and an empty list: «Шашлыки 2», for the «created» frame. */
const CREATE_EMPTY = `
  ${GP} ${wait}
  const { call } = await import('/src/shared/api/ipc.ts')
  const contacts = await call('list_contacts')
  const pick = (name) => contacts.find((contact) => contact.name === name)
  const view = await call('create_shared_event', { input: {
    title: 'Шашлыки 2', note: '', starts_on: '2026-11-02', ends_on: '2026-11-04', currency: 'rub', rounding_step: '1',
    category_id: null, organizer: 'me', i_participate: true,
    participants: ['Петя', 'Вася', 'Аня'].map((name) => ({ id: null, name, contact_id: pick(name)?.id ?? null, details: null }))
      .concat([{ id: null, name: 'Оля', contact_id: null, details: { phone: '+7 912 345-67-89', messenger: '', requisites: [], note: '' } }]),
  } })
  ${REFRESH}
  await gp.$router.push('/money/together/' + view.event.id); await sleep(900); return true`

/** Every event gone: the empty list. Run alone (the next shots need the seed). */
const DELETE_ALL = `
  ${GP} ${wait}
  const { call } = await import('/src/shared/api/ipc.ts')
  for (const row of await call('list_shared_events', { filter: { include_archived: true } })) await call('delete_shared_event', { id: row.id })
  ${REFRESH}
  await gp.$router.push('/money/together'); await sleep(900); return true`

export const together = [
  { name: 'tg-1-new-dialog', route: '/money/together', steps: [...pressText('Новое событие'), { wait: 600 }] },
  { name: 'tg-1-new-people', route: '/money/together', steps: FILL_EVENT },
  {
    name: 'tg-1-guest-details',
    route: '/money/together',
    steps: [...FILL_EVENT, ...press(ROW_MENU(2)), { wait: 300 }, ...pressText('Контакты гостя'), { wait: 600 }],
  },
  { name: 'tg-2-event-items', route: EVENT, steps: [{ wait: 600 }] },
  {
    name: 'tg-2-table',
    route: EVENT,
    steps: [{ wait: 500 }, { eval: `document.querySelector('[data-tour="shared-items-table"]').scrollIntoView({ block: 'start' }); return true` }, { wait: 300 }],
  },
  { name: 'tg-2-item-menu', route: EVENT, steps: [{ wait: 400 }, ...press(ROW_MENU(2)), { wait: 500 }] },
  { name: 'tg-2-purchase', route: EVENT, steps: [{ wait: 400 }, ...press('[data-tour="shared-buy"]'), { wait: 700 }] },
  { name: 'tg-2-unplanned', route: EVENT, steps: [{ wait: 400 }, ...pressText('Купили вне плана'), { wait: 700 }] },
  { name: 'tg-2-item-add', route: EVENT, steps: [{ wait: 400 }, ...pressText('Добавить позицию'), { wait: 600 }] },
  { name: 'tg-2-attach', route: EVENT, steps: [{ wait: 400 }, ...pressText('Привязать платёж'), { wait: 700 }] },
  {
    name: 'tg-3-split',
    route: EVENT,
    steps: [{ wait: 400 }, ...press(ROW_MENU(2)), { wait: 400 }, ...pressText('Как делить'), { wait: 700 }],
  },
  {
    name: 'tg-3-split-some',
    route: EVENT,
    steps: [{ wait: 400 }, ...press(ROW_MENU(2)), { wait: 400 }, ...pressText('Как делить'), { wait: 600 }, ...press('#split-some'), { wait: 400 }],
  },
  { name: 'tg-4-people', route: `${EVENT}?tab=people`, steps: [{ wait: 800 }] },
  {
    name: 'tg-4-override',
    route: `${EVENT}?tab=people`,
    steps: [{ wait: 600 }, ...press(ROW_MENU(2)), { wait: 400 }, ...pressText('Ручной итог'), { wait: 600 }],
  },
  { name: 'tg-5-settle', route: `${EVENT}?tab=settle`, steps: [{ wait: 800 }] },
  {
    name: 'tg-6-edit',
    route: EVENT,
    steps: [{ wait: 500 }, ...press('[data-tour="shared-event-menu"] button'), { wait: 400 }, ...pressText('Изменить событие'), { wait: 700 }],
  },
  { name: 'tg-7-created', route: '/money/together', steps: [{ eval: CREATE_EMPTY }] },
  { name: 'tg-0-list', route: '/money/together', steps: [{ wait: 700 }] },
  { name: 'tg-0-empty', route: '/money/together', steps: [{ eval: DELETE_ALL }] },
]
