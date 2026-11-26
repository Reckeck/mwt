import { expect, test } from '../harness/app'

/**
 * A new address on a vault with a hand-kept crypto wallet (spec E11), on the mock: the plan names the capital
 * that leaves the totals, the move archives the wallet (kept, not deleted) and the connected address shows on
 * the positions screen. The step-by-step wizard (security raise) has its own screen and flow.
 */
const ADDRESS = '0x52908400098527886E0F7030069857D2E4169EE7'

for (const width of [1280, 390] as const) {
  test(`connecting an address retires the hand-kept wallet and the address is shown, ${width}px`, async ({ app }) => {
    await app.unlock()
    const plan = await app.page.eval<{ names: string[]; total: string; archived: boolean; connected: boolean }>(
      `const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name
       const { call } = await import(urlOf('shared/api/ipc.ts'))
       const { queryClient } = await import(urlOf('app/providers/query/index.ts'))
       const demo = await import(urlOf('shared/api/mock/demo.ts'))
       // A run before this one may have archived the hand-kept wallet.
       for (const store of demo.mockStores) if (store.name === 'Ledger') store.archived = false
       const before = await call('crypto_transition_plan')
       const known = await call('crypto_list_accounts')
       if (!known.some((account) => account.address === '${ADDRESS.toLowerCase()}')) {
         await call('crypto_connect_address', { input: { address: '${ADDRESS}', chain: 'ethereum', name: 'Followed' } })
       }
       await call('crypto_archive_manual_wallets')
       const tree = await call('get_store_tree')
       const ledger = tree.find((node) => node.store.name === 'Ledger')
       await queryClient.resetQueries()
       return {
         names: before.wallets.map((wallet) => wallet.name),
         total: before.leaving_base,
         archived: ledger === undefined || ledger.store.archived === true,
         connected: (await call('crypto_list_accounts')).length > 0,
       }`,
    )
    expect(plan.names).toContain('Ledger')
    expect(Number(plan.total)).toBeGreaterThanOrEqual(412300)
    expect(plan.archived).toBe(true)
    expect(plan.connected).toBe(true)

    await app.at('/crypto/positions', { width, theme: 'light' })
    await app.waitFor(`return document.querySelector('[data-crypto-address-card]') !== null`, 'the connected address')
    await app.screenshot(width === 390 ? 'crypto-transition-1-address-390' : 'crypto-transition-1-address')
    await app.waitFor(
      `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`,
      'no horizontal scroll',
    )
  })
}
