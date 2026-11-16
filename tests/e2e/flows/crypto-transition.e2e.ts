import { expect, test } from '../harness/app'

/**
 * A new address on a vault below level 3 -> the wizard (step 1 names the capital that leaves the totals)
 * -> the move -> the hand-kept wallet is archived and the address is connected (spec E11), on the mock.
 */
const STRONG = 'Correct-Horse-Battery-9!'
const ADDRESS = '0x52908400098527886E0F7030069857D2E4169EE7'

for (const width of [1280, 390] as const) {
  test(`a new address starts the move, the old wallet is archived and the address is connected, ${width}px`, async ({
    app,
  }) => {
    await app.unlock()
    // Back to the start of the mock vault: password-only level 2, nothing archived yet.
    await app.page.eval(
      `// The module instances the app loaded (Vite may have added a ?t= stamp to their URLs).
     const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name
     const level = await import(urlOf('shared/api/mock/commands-security-state.ts'))
     const data = await import(urlOf('shared/api/mock/vault-data.ts'))
     const { queryClient } = await import(urlOf('app/providers/query/index.ts'))
     const demo = await import(urlOf('shared/api/mock/demo.ts'))
     // A run before this one may have archived the hand-kept wallet.
     for (const store of demo.mockStores) if (store.name === 'Ledger') store.archived = false
     // The wizard's state is one object for the whole app: another flow may have left it interrupted.
     const wizard = await import(urlOf('shared/lib/raise-wizard-state.ts'))
     Object.assign(wizard.useRaiseWizardState(), { open: false, interrupted: false })
     level.mockSecurityLevel.value = null
     data.mockPassword.value = 'secret'
     await queryClient.resetQueries()
     return true`,
    )
    await app.at('/stores', { width, theme: 'light' })

    // The empty-list card is gone once an address is followed (the other width ran first): the wallet wizard does the same.
    await app.waitFor(
      `return document.querySelector('[data-tour="crypto-add-address"]') !== null || document.querySelector('[data-tour="stores-create"]') !== null`,
      'a way to connect an address',
    )
    if (await app.exists('[data-tour="crypto-add-address"]')) await app.click('[data-tour="crypto-add-address"]')
    else {
      await app.click('[data-tour="stores-create"]')
      await app.click('[data-kind="crypto"]')
      await app.click('[data-wizard-next]')
    }
    const submit = 'button[type="submit"][form="crypto-add-address-form"]'
    // The one-time consent comes first when it was not given.
    await app.waitFor(`return document.querySelector('${submit}') !== null`, 'the dialog')
    if (!(await app.exists('#crypto-address'))) {
      await app.click(submit)
      await app.waitFor(`return document.querySelector('#crypto-address') !== null`, 'the address step')
    }
    await app.type('#crypto-address', ADDRESS)
    await app.click(submit)
    await app.waitFor(`return document.querySelector('#crypto-start-from') !== null`, 'the start step')
    await app.click(submit)

    // Step 1 of the wizard: the capital that leaves the totals and the wallet by name.
    const dialog = '[data-raise-wizard]'
    await app.waitFor(`return document.querySelector('${dialog}')?.dataset.step === '1'`, 'wizard step 1')
    await app.waitFor(`return document.querySelector('[data-raise-capital]') !== null`, 'the capital warning')
    expect(await app.text('[data-raise-capital-total]')).toMatch(/412/)
    expect(await app.text('[data-raise-capital]')).toMatch(/Ledger/)
    await app.screenshot(width === 390 ? 'crypto-transition-1-warning-390' : 'crypto-transition-1-warning')
    await app.waitFor(
      `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`,
      'no horizontal scroll',
    )
    await app.click('[data-raise-next]')

    await app.waitFor(`return document.querySelector('${dialog}')?.dataset.step === '2'`, 'step 2')
    await app.type('#raise-pin', 'secret')
    await app.type('#raise-password', STRONG)
    await app.type('#raise-repeat', STRONG)
    await app.click('[data-raise-next]')
    await app.waitFor(`return document.querySelector('${dialog}')?.dataset.step === '3'`, 'step 3')
    await app.click('[data-raise-start]')
    await app.waitFor(`return document.querySelector('${dialog}')?.dataset.step === '5'`, 'step 5')
    await app.waitFor(`return document.querySelector('[data-raise-archived]') !== null`, 'the archive line')
    expect(await app.text('[data-raise-archived]')).toMatch(/Ledger/)
    await app.screenshot(width === 390 ? 'crypto-transition-2-done-390' : 'crypto-transition-2-done')

    // Closing the wizard connects the address; the wallet is archived (kept, not deleted).
    await app.click('[data-raise-finish]')
    await app.waitFor(`return document.querySelector('${dialog}') === null`, 'the wizard to close')
    await app.waitFor(
      `const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name
     const { call } = await import(urlOf('shared/api/ipc.ts'))
     const accounts = await call('crypto_list_accounts')
     const tree = await call('get_store_tree')
     const ledger = tree.find((node) => node.store.name === 'Ledger')
     return accounts.length > 0 && (ledger === undefined || ledger.store.archived === true)`,
      'the address connected and the Ledger archived',
    )
  })
}
