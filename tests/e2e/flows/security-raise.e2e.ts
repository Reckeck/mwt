import { expect, test } from '../harness/app'

/**
 * Bybit -> wizard -> level 3 on the mock (Security wizard 1). The mock vault answers `level_required`
 * for the wallet's key until the raise runs; the raise itself is the mock command.
 */
const STRONG = 'Correct-Horse-Battery-9!'

for (const [width, theme] of [
  [1280, 'light'],
  [390, 'light'],
] as const) {
  test(`raise to level 3 from the Bybit key form, ${width}px ${theme}`, async ({ app }) => {
    await app.unlock()
    // The mock is at level 2 with a stored key that cannot be used below level 3.
    const storeId = await app.page.eval<string>(
      `// The module instances the app loaded (Vite may have added a ?t= stamp to their URLs).
       const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name
       const { call } = await import(urlOf('shared/api/ipc.ts'))
       const keys = await import(urlOf('shared/api/mock/commands-security-keys.ts'))
       const level = await import(urlOf('shared/api/mock/commands-security-state.ts'))
       const data = await import(urlOf('shared/api/mock/vault-data.ts'))
       const biometric = await import(urlOf('shared/api/mock/commands-security-biometric.ts'))
       biometric.mockBiometric.set('unavailable')
       const { queryClient } = await import(urlOf('app/providers/query/index.ts'))
       // Back to the start of the mock vault (a run before this one raised it) and refetch cached answers.
       // reset, not clear: the shell outlives this test and its observers must stay attached to the cache.
       keys.mockKeyLevelRequired.value = true
       level.mockSecurityLevel.value = null
       data.mockPassword.value = 'secret'
       await queryClient.resetQueries()
       const tree = await call('get_store_tree')
       return tree.find((node) => node.store.name === 'Bybit').store.id`,
    )
    await app.at(`/stores/${storeId}/settings?tab=exchange`, { width, theme })
    const prefix = `security-raise-${width}-${theme}`
    const dialog = '[data-raise-wizard]'
    const noOverflow = `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`

    await app.waitFor(
      `return document.querySelector('[data-tour="security-raise-start"]') !== null`,
      'the raise button',
    )
    await app.click('[data-tour="security-raise-start"]')
    await app.waitFor(`return document.querySelector('${dialog}')?.dataset.step === '1'`, 'step 1')
    await app.screenshot(`${prefix}-1-warning`)

    // Interrupting asks first and leaves a banner with a way back.
    await app.press('Escape')
    await app.waitFor(`return document.querySelector('[data-raise-stop]') !== null`, 'the interruption question')
    await app.screenshot(`${prefix}-2-interrupt`)
    await app.click('[data-raise-stop]')
    await app.waitFor(`return document.querySelector('${dialog}') === null`, 'the wizard to close')
    expect(await app.text('[data-tour="security-raise-start"]')).toMatch(/continue the move/i)
    await app.screenshot(`${prefix}-3-interrupted`)

    // A new run starts at step 2.
    await app.click('[data-tour="security-raise-start"]')
    await app.waitFor(
      `return document.querySelector('${dialog}')?.dataset.step === '2'`,
      'step 2 after an interruption',
    )

    // Empty fields: every error under its field, nothing moves on.
    await app.click('[data-raise-next]')
    await app.waitFor(`return document.querySelectorAll('[data-invalid]').length >= 3`, 'errors under the fields')
    expect(await app.page.eval<string>(`return document.activeElement?.id ?? ''`)).toBe('raise-pin')
    await app.screenshot(`${prefix}-4-errors`)

    // A weak password: the length error, then a wrong PIN comes back from the backend.
    await app.type('#raise-pin', '000000')
    await app.type('#raise-password', 'short')
    await app.type('#raise-repeat', 'other')
    await app.click('[data-raise-next]')
    await app.waitFor(`return /at least 16/i.test(document.querySelector('${dialog}').textContent)`, 'the length error')
    await app.type('#raise-password', STRONG)
    await app.type('#raise-repeat', STRONG)
    await app.waitFor(`return /strong/i.test(document.querySelector('[data-raise-strength]').textContent)`, 'strong')
    await app.screenshot(`${prefix}-5-strong`)
    await app.click('[data-raise-next]')
    await app.waitFor(`return document.querySelector('${dialog}')?.dataset.step === '3'`, 'step 3')
    await app.screenshot(`${prefix}-6-backup`)
    await app.click('[data-raise-start]')
    await app.waitFor(
      `return document.querySelector('${dialog}')?.dataset.step === '2'`,
      'back to the PIN after a wrong one',
    )
    await app.waitFor(
      `return /wrong pin/i.test(document.querySelector('${dialog}').textContent)`,
      'the wrong PIN message',
    )
    expect(await app.page.eval<string>(`return document.activeElement?.id ?? ''`)).toBe('raise-pin')

    // The right PIN (the mock's password) goes through the three stages.
    await app.type('#raise-pin', 'secret')
    await app.type('#raise-password', STRONG)
    await app.type('#raise-repeat', STRONG)
    await app.click('[data-raise-next]')
    await app.waitFor(`return document.querySelector('${dialog}')?.dataset.step === '3'`, 'step 3 again')
    await app.click('[data-raise-start]')
    await app.waitFor(`return document.querySelector('${dialog}')?.dataset.step === '4'`, 'step 4')
    expect(await app.page.eval<number>(`return document.querySelectorAll('[data-raise-stages] li').length`)).toBe(3)
    await app.screenshot(`${prefix}-7-run`)
    await app.waitFor(`return document.querySelector('${dialog}')?.dataset.step === '5'`, 'step 5')
    await app.screenshot(`${prefix}-8-done`)
    await app.waitFor(noOverflow, 'no horizontal scroll')

    await app.click('[data-raise-finish]')
    await app.waitFor(`return document.querySelector('${dialog}') === null`, 'the wizard to close')
    // The notice is gone: level 3 lets the key be used.
    await app.waitFor(
      `return document.querySelector('[data-tour="security-raise-start"]') === null`,
      'the notice to go',
    )
    await app.screenshot(`${prefix}-9-key-form`)

    // The levels card says level 3 and, without keys on the device, that there are none.
    await app.at('/?settings=vault', { width, theme })
    await app.waitFor(
      `return document.querySelector('[data-tour="security-level"]')?.dataset.level !== undefined || /level 3/i.test(document.querySelector('[data-tour="security-level"]')?.textContent ?? '')`,
      'the levels card',
    )
    expect(await app.text('[data-tour="security-level"]')).toMatch(/Level 3/)
    await app.page.eval(
      `document.querySelector('[data-tour="security-level"]').scrollIntoView({ block: 'center' }); return true`,
    )
    await app.screenshot(`${prefix}-10-levels`)
  })
}
