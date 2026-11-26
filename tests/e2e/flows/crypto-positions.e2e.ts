import { expect, test } from '../harness/app'

test('the DeFi positions screen lists the open positions with their worth', async ({ app }) => {
  await app.unlock()
  await app.at('/crypto/positions')
  await app.waitFor(`return document.querySelector('[data-position-row]') !== null`, 'a position row')
  expect(await app.exists('[data-route-placeholder]')).toBe(false)
  expect(await app.exists('[data-positions-total]')).toBe(true)
  expect(await app.exists('[data-position-value]')).toBe(true)
})

test('the closed positions stay hidden until the switch is on', async ({ app }) => {
  await app.unlock()
  await app.at('/crypto/positions')
  await app.waitFor(`return document.querySelector('[data-position-row]') !== null`, 'a position row')
  expect(await app.exists('[data-position-closed]')).toBe(false)
  await app.click('#positions-show-closed')
  await app.waitFor(`return document.querySelector('[data-position-closed]') !== null`, 'the closed position to appear')
})

test('a protocol filter narrows the list', async ({ app }) => {
  await app.unlock()
  await app.at('/crypto/positions')
  await app.waitFor(`return document.querySelector('[data-protocol="lido"]') !== null`, 'the protocol filter')
  await app.click('[data-protocol="lido"]')
  await app.waitFor(`return document.querySelectorAll('[data-position-row]').length === 1`, 'only the lido position')
})

test('Investments shows the DeFi card and it opens the positions', async ({ app }) => {
  await app.unlock()
  await app.at('/money/investments')
  await app.waitFor(`return document.querySelector('[data-invest-defi]') !== null`, 'the DeFi card')
  expect(await app.exists('[data-route-placeholder]')).toBe(false)
  await app.click('[data-invest-defi-open]')
  await app.waitFor(`return document.querySelector('[data-positions-total]') !== null`, 'the positions screen')
})
