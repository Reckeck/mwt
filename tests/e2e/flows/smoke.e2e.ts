import { expect, test } from '../harness/app'

// The screens are being rebuilt (T72): the shell and the shared route placeholder are what is left to walk.
test('unlock, home shows the shell and the route placeholder', async ({ app }) => {
  await app.unlock()
  await app.at('/')
  for (const anchor of ['quick-add', 'open-settings', 'command-palette'])
    expect(await app.exists(`[data-tour="${anchor}"]`), anchor).toBe(true)
})

test('a route of a deleted screen still resolves and shows the placeholder', async ({ app }) => {
  await app.unlock()
  await app.at('/transactions')
  await app.waitFor(`return document.querySelector('[data-tour="quick-add"]') !== null`, 'the shell to mount')
  await app.waitFor(`return document.querySelector('[data-route-placeholder]') !== null`, 'the route placeholder')
  expect(await app.exists('[data-route-placeholder]')).toBe(true)
  expect(await app.exists('[role="dialog"]')).toBe(false)
})
