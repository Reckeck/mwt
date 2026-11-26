import { expect, test } from '../harness/app'

/**
 * What the module shows once an address is followed, on the mock: the address card and its network-settings
 * entry, the quarantine with its review, and the price chart, all on the DeFi positions screen. The wallet page,
 * the operations screen and the wipe entry belong to other screens and have their own flows.
 */
const ADDRESS = '0x52908400098527886E0F7030069857D2E4169EE7'

async function follow(app: { page: { eval<T>(code: string): Promise<T> } }) {
  await app.page.eval(
    `const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name
     const { call } = await import(urlOf('shared/api/ipc.ts'))
     const { queryClient } = await import(urlOf('app/providers/query/index.ts'))
     const known = await call('crypto_list_accounts')
     if (!known.some((account) => account.address === '${ADDRESS.toLowerCase()}')) {
       await call('crypto_connect_address', { input: { address: '${ADDRESS}', chain: 'ethereum', name: 'Followed' } })
     }
     await queryClient.resetQueries()
     return true`,
  )
}

for (const width of [1280, 390] as const) {
  test(`a followed address: card, settings entry, quarantine and price chart, ${width}px`, async ({ app }) => {
    await app.unlock()
    await follow(app)
    await app.at('/crypto/positions', { width, theme: 'light' })
    await app.waitFor(`return document.querySelector('[data-tour="crypto-overview"]') !== null`, 'the address card')
    expect(await app.text('[data-crypto-address]')).toMatch(/0x/)
    expect(await app.exists('[data-network-settings]')).toBe(true)
    await app.waitFor(`return document.querySelector('[data-crypto-quarantine]') !== null`, 'the quarantine block')
    expect(await app.text('[data-crypto-quarantine]')).toMatch(/PEPE2/)
    await app.waitFor(
      `return document.querySelector('[data-price-chart-plot] canvas, [data-price-chart-plot] svg') !== null`,
      'the price chart',
    )
    await app.screenshot(`crypto-engine-1-addresses-${width}`)
    await app.waitFor(
      `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`,
      'no horizontal scroll on the positions screen',
    )
  })
}

test('the network settings entry opens the Crypto settings', async ({ app }) => {
  await app.unlock()
  await follow(app)
  await app.at('/crypto/positions', { width: 1280, theme: 'light' })
  await app.waitFor(`return document.querySelector('[data-network-settings]') !== null`, 'the settings entry')
  await app.click('[data-network-settings]')
  await app.waitFor(`return new URLSearchParams(location.search).get('settings') === 'crypto'`, 'the settings query')
})

test('recognising a quarantined token takes it out of the quarantine after a review', async ({ app }) => {
  await app.unlock()
  await follow(app)
  await app.at('/crypto/positions', { width: 1280, theme: 'light' })
  await app.waitFor(`return document.querySelector('[data-quarantine-recognize]') !== null`, 'a quarantined token')
  const before = await app.page.eval<number>(`return document.querySelectorAll('[data-crypto-quarantine-row]').length`)
  await app.click('[data-quarantine-recognize]')
  await app.waitFor(`return document.querySelector('[data-token-review]') !== null`, 'the review dialog')
  await app.click('[data-review-confirm]')
  await app.waitFor(
    `return document.querySelector('[data-token-review]') === null && document.querySelectorAll('[data-crypto-quarantine-row]').length < ${before}`,
    'the token to leave the quarantine',
  )
})
