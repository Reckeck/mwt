import { type App, expect, test } from '../harness/app'

const SKEWERS = '/money/together/mock-event-skewers?tab=settle'
const POSITION = '[data-tour="shared-my-position"]'
const TABLE = '[data-tour="shared-settlement"]'

/** Page-side: the sum that follows `label` in the position block, or NaN. */
const READ_POSITION = (label: string) => `
  const text = (document.querySelector('${POSITION}')?.textContent ?? '').replace(/\\s+/g, ' ')
  const match = text.match(new RegExp(${JSON.stringify(label)} + ' ?([0-9 ,\\u00a0\\u202f]+)'))
  return match ? Number(match[1].replace(/\\D/g, '')) : NaN`

/** The sum in a `My position` card by its label, as a number. */
const position = (app: App, label: string) => app.page.eval<number>(READ_POSITION(label))

/** Click a row's button (main action or «⋯») by the person's name. */
const rowButton = (name: string, selector: string) => `
  const row = [...document.querySelectorAll('${TABLE} tr')].find((entry) => entry.textContent.includes('${name}'))
  row.querySelector(${JSON.stringify(selector)}).click()
  return true`

async function openSettlement(app: App) {
  await app.unlock()
  await app.at(SKEWERS)
  await app.waitFor(`return document.querySelector('${POSITION}') !== null`, 'my position')
}

async function waitOwed(app: App, amount: number) {
  await app.waitFor(
    `const read = async () => { ${READ_POSITION('Owed to me')} }; return (await read()) === ${amount}`,
    `«Owed to me» to be ${amount}`,
  )
}

test('the settlement starts with the whole debt owed to me', async ({ app }) => {
  await openSettlement(app)
  expect(await position(app, 'Owed to me')).toBe(5540)
  expect(await position(app, 'Paid')).toBe(6600)
  expect(await position(app, 'My share')).toBe(1060)
})

test('Petya pays back in full, then Vasya in part and the rest is written off: 5540 > 3805 > 2120', async ({ app }) => {
  await openSettlement(app)

  // Petya returns his 1735.
  await app.page.eval(rowButton('Петя', 'button:not([aria-label])'))
  await app.waitFor(`return document.querySelector('[data-overlay="shared-settle"]') !== null`, 'the «Received» dialog')
  expect((await app.value('#shared-settle-amount')).replace(/\s/g, '')).toBe('1735')
  await app.clickText('[data-overlay="shared-settle"] button', '^Record$')
  await app.waitFor(`return document.querySelector('[data-overlay="shared-settle"]') === null`, 'the dialog to close')
  await waitOwed(app, 3805)
  expect(await position(app, 'Owed to me')).toBe(3805)
  expect(await position(app, 'Returned')).toBe(1735)

  // Vasya returns 1000 of his 1685 («less is fine»).
  await app.page.eval(rowButton('Вася', 'button:not([aria-label])'))
  await app.waitFor(`return document.querySelector('[data-overlay="shared-settle"]') !== null`, 'the dialog')
  expect((await app.value('#shared-settle-amount')).replace(/\s/g, '')).toBe('1685')
  await app.type('#shared-settle-amount', '1000')
  await app.clickText('[data-overlay="shared-settle"] button', '^Record$')
  await app.waitFor(`return document.querySelector('[data-overlay="shared-settle"]') === null`, 'the dialog to close')
  await waitOwed(app, 2805)
  expect(await app.text(TABLE)).toMatch(/Вася[\s\S]*partly/)

  // The remaining 685 is written off through «⋯».
  await app.page.eval(rowButton('Вася', 'button[aria-label="More"]'))
  await app.clickText('[role="menuitem"]', 'Write off')
  await app.waitFor(
    `return document.querySelector('[data-overlay="shared-remainder"]') !== null`,
    'the write-off dialog',
  )
  expect(await app.text('[data-overlay="shared-remainder"]')).toContain('685')
  await app.clickText('[data-overlay="shared-remainder"] button', '^Write off$')
  await app.waitFor(
    `return document.querySelector('[data-overlay="shared-remainder"]') === null`,
    'the dialog to close',
  )
  await waitOwed(app, 2120)
  expect(await position(app, 'Written off')).toBe(685)
})

test('the settlement share text names the event and the people', async ({ app }) => {
  await openSettlement(app)
  await app.click('[data-tour="shared-share"]')
  await app.waitFor(
    `return (document.querySelector('[data-overlay="shared-share"] pre')?.textContent ?? '').length > 20`,
    'the share text',
  )
  const text = await app.text('[data-overlay="shared-share"] pre')
  expect(text).toContain('Шашлыки')
  expect(text).toContain('Петя')
  expect(text).toMatch(/1[\s  ]?735/)
})

/** An event Anna leads: she bought a 900 item for the three of us (me, Anna, Boris), so my share is 300. */
async function makeEventLedByAnna(app: App): Promise<string> {
  return app.page.eval<string>(`
    const { call } = await import('/src/shared/api/ipc.ts')
    const guest = (id, name) => ({ id, name, contact_id: null, details: null })
    const view = await call('create_shared_event', { input: {
      title: 'Дача', note: '', starts_on: null, ends_on: null, currency: 'rub', rounding_step: '1', category_id: null,
      organizer: 'anna', i_participate: true, participants: [guest('anna', 'Анна'), guest('boris', 'Борис')],
    } })
    await call('add_shared_purchase', { input: { event_id: view.event.id, purchase: {
      item_id: null, new_item_title: 'Продукты', new_item_unit: 'шт', buyer: 'anna', qty: '1', amount: '900', currency: 'rub',
      price_mode: 'total', rate_to_event: null, at: null, store_id: null,
    } } })
    return view.event.id`)
}

test('an event somebody else leads: «Pay my share» pays the organizer and my debt goes away', async ({ app }) => {
  await app.unlock()
  await app.at('/')
  const id = await makeEventLedByAnna(app)
  await app.at(`/money/together/${id}?tab=settle`)
  await app.waitFor(`return document.querySelector('[data-tour="shared-pay-share"]') !== null`, '«Pay my share»')
  expect(await app.text(POSITION)).toMatch(/To pay Анна\s*300/)

  await app.click('[data-tour="shared-pay-share"]')
  await app.waitFor(`return document.querySelector('[data-overlay="shared-pay-share"]') !== null`, 'the pay dialog')
  expect((await app.value('#shared-pay-amount')).replace(/\s/g, '')).toBe('300')
  // The recipient is the organizer and cannot be changed: no field for it, just her name.
  expect(await app.text('[data-overlay="shared-pay-share"]')).toContain('Анна')
  await app.clickText('[data-overlay="shared-pay-share"] button', '^Pay$')
  await app.waitFor(
    `return document.querySelector('[data-overlay="shared-pay-share"]') === null`,
    'the dialog to close',
  )

  // Paid in full: nothing left to pay, the button is gone and the position shows what I gave back.
  await app.waitFor(`return document.querySelector('[data-tour="shared-pay-share"]') === null`, 'the pay button to go')
  expect(await app.text(POSITION)).toMatch(/I gave back\s*300/)
})
