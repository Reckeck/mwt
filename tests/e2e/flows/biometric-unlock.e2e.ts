import { expect, test } from '../harness/app'

const SETTINGS = '/?settings=vault'
const TOGGLE = '[data-tour="biometric-toggle"]'

/** What the "device" offers and the level of the mock vault, set from the page (the mock has no OS). */
const setDevice = (state: string, level: string | null) => `
  // The module instance the app loaded (Vite may have added a ?t= stamp to its URL).
  const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name))
  const biometric = await import(urlOf('commands-security-biometric.ts'))
  const security = await import(urlOf('commands-security-state.ts'))
  // Another scenario may have changed the mock password (a raise sets a new one).
  const data = await import(urlOf('mock/vault-data.ts'))
  data.mockPassword.value = 'secret'
  biometric.mockBiometric.set(${JSON.stringify(state)})
  security.mockSecurityLevel.value = ${JSON.stringify(level)}
  const queryClient = document.querySelector('#app').__vue_app__._context.provides.VUE_QUERY_CLIENT
  await queryClient.invalidateQueries()
  return true`

/** The card sits below the first screenful of the section: bring it into the shot. */
const showCard = `document.querySelector('[data-section="vault"] [data-biometric-status], [data-biometric-unavailable]')?.scrollIntoView({ block: 'center' }); return true`

for (const width of [1280, 390] as const) {
  test(`biometric card and sign-in button, ${width}px`, async ({ app }) => {
    await app.unlock()
    // The first dynamic import of the mock may make Vite reload the page: do it before the scenario starts.
    await app.page.eval(setDevice('available', 'crypto')).catch(() => undefined)
    await app.unlock()

    // Level 3 with a hardware-bound store: the switch asks for the password, then it is on.
    await app.at(SETTINGS, { width, theme: 'light' })
    await app.page.eval(setDevice('available', 'crypto'))
    await app.waitFor(`return document.querySelector('${TOGGLE}') !== null`, 'the biometric switch')
    await app.screenshot(`biometric-${width}-1-available`)
    await app.click(`${TOGGLE} button[role="switch"]`)
    await app.waitFor(`return document.querySelector('#biometric-password') !== null`, 'the password dialog')
    await app.screenshot(`biometric-${width}-2-password`)
    await app.type('#biometric-password', 'secret')
    await app.clickText('button', '^Turn on$')
    await app.waitFor(
      `return /^On\\./.test(document.querySelector('[data-biometric-status]')?.textContent?.trim() ?? '')`,
      'enabled',
    )
    await app.page.eval(showCard)
    await app.screenshot(`biometric-${width}-3-enabled`)

    // The 7 days are over: the switch stays on and says a password is needed.
    await app.page.eval(setDevice('password_required', 'crypto'))
    await app.waitFor(
      `return /Password needed/.test(document.querySelector('[data-biometric-status]')?.textContent ?? '')`,
      'expired',
    )
    await app.page.eval(showCard)
    await app.screenshot(`biometric-${width}-4-expired`)

    // No hardware-bound store: no switch, the card stays and explains.
    await app.page.eval(setDevice('unavailable', 'crypto'))
    await app.waitFor(`return document.querySelector('[data-biometric-unavailable]') !== null`, 'the explanation')
    expect(await app.exists(TOGGLE)).toBe(false)
    await app.page.eval(showCard)
    await app.screenshot(`biometric-${width}-5-unavailable`)
    expect(
      await app.page.eval<boolean>(
        `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`,
      ),
    ).toBe(true)

    // Below level 3 there is no card at all.
    await app.page.eval(setDevice('available', null))
    await app.waitFor(
      `return document.querySelector('[data-biometric-unavailable]') === null && document.querySelector('${TOGGLE}') === null`,
      'no card',
    )
  })
}
