import { expect, test } from '../harness/app'

test('the accounts screen shows the hero, what capital is made of and accounts by purpose', async ({ app }) => {
  await app.unlock()
  await app.at('/stores')
  await app.waitFor(`return document.querySelector('[data-net-worth-total]') !== null`, 'the capital hero')
  expect(await app.exists('[data-route-placeholder]')).toBe(false)
  expect(await app.exists('[data-net-worth-chart]')).toBe(true)
  expect(await app.exists('[data-allocation]')).toBe(true)
  expect(await app.exists('[data-purpose-group] [data-store-row]')).toBe(true)
})

test('the range switch picks another span of the capital line', async ({ app }) => {
  await app.unlock()
  await app.at('/stores')
  await app.waitFor(`return document.querySelector('[data-range="last3"]') !== null`, 'the range switch')
  await app.click('[data-range="last3"]')
  await app.waitFor(
    `return document.querySelector('[data-range="last3"]')?.getAttribute('data-state') === 'on'`,
    'the span to be lit',
  )
})

test('an account row opens its page', async ({ app }) => {
  await app.unlock()
  await app.at('/stores')
  await app.waitFor(`return document.querySelector('[data-store-row]') !== null`, 'an account row')
  await app.click('[data-store-row]')
  await app.waitFor(
    `return /^\\/stores\\/[^/]+$/.test(location.pathname + location.hash.replace('#', ''))
    || document.querySelector('#app')?.__vue_app__?.config.globalProperties.$route.name === 'wallet'`,
    'the account page',
  )
})

test('Money · Capital shows the same hero in both themes', async ({ app }) => {
  await app.unlock()
  for (const theme of ['light', 'dark'] as const) {
    await app.at('/money/wealth', { theme })
    await app.waitFor(`return document.querySelector('[data-net-worth-total]') !== null`, `the hero in ${theme}`)
    expect(await app.exists('[data-allocation]')).toBe(true)
  }
})
