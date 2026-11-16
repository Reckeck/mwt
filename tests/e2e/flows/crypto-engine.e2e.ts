import { expect, test } from '../harness/app'

/**
 * The path after an address is connected (spec E16), on the mock: the wallet with its assets and the quarantine,
 * the operations screen, and the Settings entry that offers the wipe. Connecting itself is crypto-transition.e2e.ts.
 */
const ADDRESS = '0x52908400098527886E0F7030069857D2E4169EE7'

for (const width of [1280, 390] as const) {
  test(`a followed address: assets, quarantine, operations and the wipe entry, ${width}px`, async ({ app }) => {
    await app.unlock()
    const storeId = await app.page.eval<string>(
      `const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name
     const { call } = await import(urlOf('shared/api/ipc.ts'))
     const { queryClient } = await import(urlOf('app/providers/query/index.ts'))
     const known = await call('crypto_list_accounts')
     let id = known.find((account) => account.address === '${ADDRESS.toLowerCase()}')?.store_id
     if (!id) {
       id = (await call('crypto_connect_address', { input: { address: '${ADDRESS}', chain: 'ethereum', name: 'Followed' } })).store_id
     }
     await queryClient.resetQueries()
     return id`,
    )

    await app.at(`/stores/${storeId}`, { width, theme: 'light' })
    await app.waitFor(`return document.querySelector('[data-tour="crypto-overview"]') !== null`, 'the address card')
    await app.waitFor(`return document.querySelector('[data-crypto-quarantine]') !== null`, 'the quarantine block')
    expect(await app.text('[data-crypto-quarantine]')).toMatch(/PEPE2/)
    // The assets block lists what the address holds, not an empty state.
    const assets = await app.text('[data-tour="crypto-assets"]')
    expect(assets).toMatch(/ETH/)
    expect(assets).toMatch(/USDT/)
    expect(assets).not.toMatch(/appear after the first read/i)
    // 0.0474 ETH + 500 USDT at the mock's quotes (3000 and 1): the header total is 642.20, never 0.00.
    expect(await app.text('main')).toMatch(/642[.,]2/)
    await app.screenshot(`crypto-engine-1-wallet-${width}`)
    await app.waitFor(
      `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`,
      'no horizontal scroll on the wallet',
    )

    await app.at('/transactions/crypto', { width, theme: 'light' })
    await app.waitFor(`return document.querySelector('[data-tour="crypto-operations"]') !== null`, 'the operations')
    await app.waitFor(`return document.querySelectorAll('[data-crypto-op]').length >= 5`, 'operation rows')
    const kinds = await app.page.eval<string[]>(
      `return [...new Set([...document.querySelectorAll('[data-tx-kind]')].map((row) => row.textContent.trim()))]`,
    )
    expect(kinds.join(' ')).toMatch(/transfer/i)
    expect(kinds.join(' ')).toMatch(/swap|exchange/i)
    await app.screenshot(`crypto-engine-2-operations-${width}`)
    await app.waitFor(
      `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`,
      'no horizontal scroll on the operations',
    )

    // A search that matches nothing is answered by the empty state, not by a blank card.
    await app.type('input[type="search"], input[placeholder*="address" i]', 'zzzz-no-such-address')
    await app.waitFor(
      `return document.querySelectorAll('[data-crypto-op]').length === 0 && /Nothing found/.test(document.body.innerText)`,
      'the empty state of a filter that matches nothing',
    )
    await app.screenshot(`crypto-engine-2b-operations-empty-${width}`)

    await app.at('/?settings=crypto&section=crypto-wipe', { width, theme: 'light' })
    await app.waitFor(`return document.querySelector('[data-tour="crypto-wipe"]') !== null`, 'the wipe entry')
    expect(await app.exists('[data-tour="crypto-wipe-backup"]')).toBe(true)
    await app.screenshot(`crypto-engine-3-wipe-${width}`)
  })
}
