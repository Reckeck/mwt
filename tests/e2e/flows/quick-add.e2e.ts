import { afterEach } from 'vitest'

import { connect, expect, test, type App } from '../harness/app'

const DIALOG = '[data-overlay="quick-add"]'
const AMOUNT = '#quick-amount'

async function openQuickAdd(app: App) {
  await app.click('[data-tour="quick-add"]')
  await app.waitFor(`return document.querySelector('${DIALOG} #quick-store') !== null`, 'the quick add form')
  // The wallet starts on the first one, once the wallets are loaded.
  await app.waitFor(
    `return document.querySelector('${DIALOG} #quick-store')?.textContent.trim() !== ''`,
    'a wallet in the field',
  )
}

async function pickFirstCategory(app: App) {
  // Reka opens a select on pointerdown, not on click.
  await app.page.eval(
    `document.querySelector('#quick-category').dispatchEvent(new PointerEvent('pointerdown', { button: 0, ctrlKey: false, bubbles: true, pointerType: 'mouse' })); return true`,
  )
  await app.waitFor(`return document.querySelector('[role="option"]') !== null`, 'the category list')
  await app.page.eval(
    `const option = document.querySelector('[role="option"]'); option.dispatchEvent(new PointerEvent('pointerup', { button: 0, bubbles: true, pointerType: 'mouse' })); option.click(); return true`,
  )
}

// The page is shared by the tests of a run: a form left open would sit over the next screen.
afterEach(async () => {
  const app = await connect()
  try {
    await app.page.eval(
      `const app = document.querySelector('#app').__vue_app__; const { quickAddOpen } = await import('/src/shared/lib/quick-add.ts'); quickAddOpen.value = false; return true`,
    )
  } finally {
    await app.close()
  }
})

const closed = (app: App) =>
  app.waitFor(`return document.querySelector('${DIALOG}') === null`, 'the quick add to close')

test('desktop: save validates on save, then writes and closes', async ({ app }) => {
  await app.unlock()
  await app.at('/')
  await openQuickAdd(app)
  expect(await app.exists(`${DIALOG} [data-quick-kind]`)).toBe(true)
  // The pad is for a phone only.
  expect(await app.exists(`${DIALOG} [data-amount-pad]`)).toBe(false)

  // Save stays active: an empty form says what is missing under the fields and stays open.
  await app.click('[data-quick-add-submit]')
  await app.waitFor(`return document.querySelector('${DIALOG} [data-invalid]') !== null`, 'a message under the amount')
  expect(await app.exists(DIALOG)).toBe(true)

  // An expression typed inline becomes the amount.
  await app.type(AMOUNT, '1200+350*2')
  await app.press('Enter')
  await app.waitFor(
    `return document.querySelector('${AMOUNT}')?.value.replace(/\\s/g, '') === '1900'`,
    'the expression to be calculated',
  )

  await pickFirstCategory(app)
  await app.click('[data-quick-add-submit]')
  await closed(app)
})

test('desktop: kind switch shows the destination of a transfer instead of a category', async ({ app }) => {
  await app.unlock()
  await app.at('/')
  await openQuickAdd(app)
  expect(await app.exists('#quick-category')).toBe(true)
  await app.click('[data-kind="transfer"]')
  await app.waitFor(`return document.querySelector('#quick-to-store') !== null`, 'the destination wallet')
  expect(await app.exists('#quick-category')).toBe(false)
  await app.click('[data-kind="topup"]')
  await app.waitFor(`return document.querySelector('#quick-category') !== null`, 'the category of an income')
})

test('desktop: Ctrl Enter saves and keeps the form for the next one', async ({ app }) => {
  await app.unlock()
  await app.at('/')
  await openQuickAdd(app)
  await app.type(AMOUNT, '250')
  await pickFirstCategory(app)
  await app.page.eval(
    `document.querySelector('#quick-add-form').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true })); return true`,
  )
  await app.waitFor(
    `return /2/.test(document.querySelector('${DIALOG} [data-dialog-body]')?.closest('[role="dialog"]')?.querySelector('h2')?.textContent ?? '')`,
    'the next one in a row',
  )
  expect(await app.value(AMOUNT)).toBe('')
  // The category stays for the next one.
  expect(await app.text('#quick-category')).not.toBe('')
  expect(await app.exists(DIALOG)).toBe(true)
})

test('phone: a sheet with the keypad that types the amount', async ({ app }) => {
  await app.unlock()
  await app.at('/', { width: 390 })
  await openQuickAdd(app)
  expect(await app.exists(`${DIALOG} [data-amount-pad]`)).toBe(true)
  for (const key of ['1', '2', '+', '3']) await app.click(`${DIALOG} [data-pad-key="${key}"]`)
  await app.waitFor(
    `return document.querySelector('${AMOUNT}')?.value.replace(/\\s/g, '') === '15'`,
    'the pad to calculate 12+3',
  )
  await app.click('[data-quick-add-submit]')
  await app.waitFor(`return document.querySelector('${DIALOG} [data-invalid]') !== null`, 'the category message')
  expect(await app.exists(DIALOG)).toBe(true)
})
