import { expect, test } from '../harness/app'

/**
 * A second address is connected without touching the name field: the default name is told apart
 * from the first wallet's ("Main"), so the backend does not refuse it with STORE_NAME_TAKEN. On the mock.
 */
const FIRST = '0x52908400098527886E0F7030069857D2E4169EE7'
const SECOND = { 1280: '0x1111111111111111111111111111111111111111', 390: '0x2222222222222222222222222222222222222222' }

for (const width of [1280, 390] as const) {
  test(`a second address connects with its default name, ${width}px`, async ({ app }) => {
    await app.unlock()
    await app.page.eval(
      `const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name
     const { call } = await import(urlOf('shared/api/ipc.ts'))
     const level = await import(urlOf('shared/api/mock/commands-security-state.ts'))
     const { queryClient } = await import(urlOf('app/providers/query/index.ts'))
     // Level 3 and consent given: the dialog goes straight to the address.
     level.mockSecurityLevel.value = 'crypto'
     await call('crypto_set_tracking_consent', { consent: true })
     const known = await call('crypto_list_accounts')
     if (!known.some((account) => account.address === '${FIRST.toLowerCase()}')) {
       await call('crypto_connect_address', { input: { address: '${FIRST}', chain: 'ethereum', name: 'Main' } })
     }
     await queryClient.resetQueries()
     return true`,
    )
    await app.at('/stores', { width, theme: 'light' })
    await app.click('[data-tour="stores-create"]')
    await app.click('[data-kind="crypto"]')
    await app.click('[data-wizard-next]')

    const submit = 'button[type="submit"][form="crypto-add-address-form"]'
    await app.waitFor(`return document.querySelector('#crypto-address') !== null`, 'the address step')
    await app.type('#crypto-address', SECOND[width])
    await app.waitFor(
      `return /^Ethereum · 0x/.test(document.querySelector('#crypto-name')?.value ?? '')`,
      'the default name',
    )
    expect(await app.value('#crypto-name')).not.toBe('Main')
    await app.click(submit)
    await app.waitFor(`return document.querySelector('#crypto-start-from') !== null`, 'the start step')
    await app.click(submit)

    await app.waitFor(
      `const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name
     const { call } = await import(urlOf('shared/api/ipc.ts'))
     const accounts = await call('crypto_list_accounts')
     return accounts.some((account) => account.address === '${SECOND[width]}')`,
      'the second address connected',
    )
    expect(await app.exists('#crypto-add-address-form')).toBe(false)
  })
}
