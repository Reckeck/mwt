import { expect, test } from '../harness/app'

const PEOPLE_FIELD = 'input[aria-label="Add a person: name or contact"]'
const CONTACT_DIALOG = '[data-overlay="contact-edit"]'
const EVENT_DIALOG = '[data-overlay="shared-event-edit"]'

test('a contact typed in a form is created on the spot, picked, and lands in the directory', async ({ app }) => {
  await app.unlock()
  // Visit the directory first: Vite may reload the page when a route first needs a dependency, which would wipe the mock.
  await app.at('/transactions/contacts')
  await app.at('/money/together')
  await app.click('[data-tour="shared-new"]')
  await app.waitFor(`return document.querySelector('${EVENT_DIALOG}') !== null`, 'the new event dialog')

  // What is already typed in the form must survive the contact dialog on top of it.
  await app.type('#event-title', 'Поездка')

  await app.type(PEOPLE_FIELD, 'Гриша')
  await app.clickText('[role="option"]', 'Create «Гриша»')
  await app.waitFor(`return document.querySelector('${CONTACT_DIALOG}') !== null`, 'the contact form over the event')
  expect(await app.value('#contact-edit-name')).toBe('Гриша')
  await app.clickText(`${CONTACT_DIALOG} button`, '^Create$')
  await app.waitFor(`return document.querySelector('${CONTACT_DIALOG}') === null`, 'the contact form to close')

  // The event form is still there with its title, and the new contact is a participant.
  expect(await app.value('#event-title')).toBe('Поездка')
  await app.waitFor(
    `return /People · 2/.test(document.querySelector('${EVENT_DIALOG}')?.textContent ?? '') && /Гриша/.test(document.querySelector('${EVENT_DIALOG}')?.textContent ?? '')`,
    'Гриша among the participants',
  )

  await app.press('Escape')
  await app.waitFor(`return document.querySelector('${EVENT_DIALOG}') === null`, 'the event dialog to close')

  // The contact is real: it is in the directory.
  await app.at('/transactions/contacts')
  await app.waitFor(
    `return /Гриша/.test(document.querySelector('main')?.textContent ?? document.body.textContent)`,
    'Гриша in the contacts directory',
  )
})
