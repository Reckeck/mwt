import { type App, expect, test } from '../harness/app'

/** Opens the quick-add form and waits for the amount field. */
async function openForm(app: App) {
  await app.click('[data-tour="quick-add"]')
  await app.waitFor(`return document.querySelector('#tx-amount') !== null`, 'the amount field')
}

test('typing an expression: the hint shows the result and accepting leaves it in the field', async ({ app }) => {
  await app.unlock()
  await app.at('/')
  await openForm(app)

  await app.type('#tx-amount', '1200/3+50')
  await app.waitFor(`return document.querySelector('[data-amount-hint]') !== null`, 'the hint under the field')
  expect(await app.text('[data-amount-hint]')).toMatch(/450/)

  // Enter makes the expression the amount: the field holds the plain number, the hint is gone.
  await app.press('Enter')
  await app.waitFor(`return document.querySelector('[data-amount-hint]') === null`, 'the hint to go once accepted')
  expect((await app.value('#tx-amount')).replace(/\s/g, '')).toBe('450')
})

test('leaving the field accepts the expression and notes it in the comment', async ({ app }) => {
  await app.unlock()
  await app.at('/')
  await openForm(app)

  await app.type('#tx-amount', '10*5-8')
  await app.press('Tab')
  await app.waitFor(
    `return (document.querySelector('#tx-amount')?.value ?? '').replace(/\\s/g, '') === '42'`,
    'the expression to become 42 on blur',
  )
  // The expression is kept in the comment so the sum can be traced later.
  expect(await app.value('#tx-comment')).toBe('10*5-8 = 42')
})

test('a phone keypad: expression, equals and done write the result', async ({ app }) => {
  await app.unlock()
  await app.at('/', { width: 390 })
  await openForm(app)

  await app.click('[data-amount-calc]')
  await app.waitFor(`return document.querySelector('[data-amount-keypad]') !== null`, 'the keypad to open')
  for (const key of ['1', '2', '0', '0', '÷', '3', '+', '5', '0'])
    await app.click(`[data-amount-keypad] [data-calc-key="${key}"]`)
  await app.waitFor(
    `return /450/.test(document.querySelector('[data-keypad-result]')?.textContent ?? '')`,
    'the keypad to show 450',
  )
  await app.click('[data-calc-done]')
  await app.waitFor(`return document.querySelector('[data-amount-keypad]') === null`, 'the keypad to close')
  expect((await app.value('#tx-amount')).replace(/\s/g, '')).toBe('450')
})
