import { expect, test } from '../harness/app'

const SHEET = '[data-tour="contact-sheet"]'
const ROUTE = `const app = document.querySelector('#app').__vue_app__; const route = app.config.globalProperties.$route;`

test('the contacts section lists contacts, opens a card on a row and closes it with Back', async ({ app }) => {
  await app.unlock()
  await app.at('/contacts')
  await app.waitFor(`return document.querySelector('[data-tour="contact-groups"] tbody tr') !== null`, 'the table rows')
  await app.waitFor(
    `return /Олег/.test(document.querySelector('[data-tour="contact-groups"]').textContent)`,
    'Олег in the table',
  )
  expect(await app.exists('[data-tour="contact-summary"]')).toBe(true)
  expect(await app.exists(SHEET)).toBe(false)

  await app.clickText('[data-tour="contact-groups"] tbody tr', 'Олег')
  await app.waitFor(
    `${ROUTE} return Boolean(route.query.contact) && document.querySelector('${SHEET}') !== null`,
    'the card to open',
  )

  // Requisites tab: «+ Requisite» puts the form first; cancel hides it.
  // Reka tabs activate on mousedown, not click.
  await app.waitFor(
    `const tab = [...document.querySelectorAll('[data-tour="contact-sheet-tabs"] [role="tab"]')].find((el) => /^Accounts/.test(el.textContent.trim()))
     if (!tab) return false
     tab.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }))
     return document.querySelector('[data-tour="contact-add-account"]') !== null`,
    'the Accounts tab',
  )
  await app.waitFor(`return document.querySelector('[data-tour="contact-add-account"]') !== null`, 'the add button')
  expect(await app.exists('[data-tour="contact-account-form"]')).toBe(false)
  await app.click('[data-tour="contact-add-account"]')
  await app.waitFor(
    `const form = document.querySelector('[data-tour="contact-account-form"]'); const list = document.querySelector('[data-tour="contact-accounts"]')
     return Boolean(form && list && list.contains(form) && form.compareDocumentPosition(list.querySelector('.divide-y')) & Node.DOCUMENT_POSITION_FOLLOWING)`,
    'the form as the first row',
  )
  await app.click('button[aria-label="Cancel"]')
  await app.waitFor(`return document.querySelector('[data-tour="contact-account-form"]') === null`, 'the form to hide')

  await app.page.eval(`history.back(); return true`)
  await app.waitFor(
    `${ROUTE} return !route.query.contact && document.querySelector('${SHEET}') === null`,
    'Back to close the card',
  )
})

test('the old transactions URL redirects to the section with the card open', async ({ app }) => {
  await app.unlock()
  await app.at('/contacts')
  await app.page.eval(
    `document.querySelector('#app').__vue_app__.config.globalProperties.$router.push('/transactions/contacts?contact=mock-contact-oleg'); return true`,
  )
  await app.waitFor(
    `${ROUTE} return route.path === '/contacts' && route.query.contact === 'mock-contact-oleg' && document.querySelector('${SHEET}') !== null`,
    'the redirect with the card open',
  )
})

test('on a phone the list renders and the card opens as a drawer', async ({ app }) => {
  await app.unlock()
  await app.at('/contacts', { width: 390 })
  await app.waitFor(
    `return /Олег/.test(document.querySelector('[data-tour="contact-groups"]')?.textContent ?? '')`,
    'Олег in the list',
  )
  expect(await app.exists('[data-tour="contact-groups"] tbody')).toBe(false)
  await app.clickText('[data-tour="contact-groups"] button', 'Олег')
  await app.waitFor(`return document.querySelector('[data-overlay="contact-sheet"]') !== null`, 'the drawer')
})
