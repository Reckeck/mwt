import { expect, test } from '../harness/app'

test('unlock, home, open the new transaction form and close it', async ({ app }) => {
  await app.unlock()
  await app.at('/')
  expect(await app.exists('[data-tour="quick-add"]')).toBe(true)

  await app.click('[data-tour="quick-add"]')
  await app.waitFor(`return document.querySelector('[role="dialog"]') !== null`, 'the quick-add dialog to open')

  await app.page.send('Input.dispatchKeyEvent', {
    type: 'keyDown',
    key: 'Escape',
    code: 'Escape',
    windowsVirtualKeyCode: 27,
  })
  await app.page.send('Input.dispatchKeyEvent', {
    type: 'keyUp',
    key: 'Escape',
    code: 'Escape',
    windowsVirtualKeyCode: 27,
  })
  await app.waitFor(`return document.querySelector('[role="dialog"]') === null`, 'the dialog to close')
})
