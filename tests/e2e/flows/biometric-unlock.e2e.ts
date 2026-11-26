import { type App, expect, test } from '../harness/app'

/** The unlock screen: Display title, recent vaults as rows, password error, biometric sign-in (the mock has no OS). */
const VAULT = `const vault = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('vault');`
const URL_OF = `const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name))`
const FORM = '[data-unlock-form]'
const TITLE = '[data-unlock-title]'

/** What the "device" offers; the mock vault password is reset because another scenario may have changed it. */
const setDevice = (state: string) => `
  ${URL_OF}
  const biometric = await import(urlOf('commands-security-biometric.ts'))
  const data = await import(urlOf('mock/vault-data.ts'))
  data.mockPassword.value = 'secret'
  biometric.mockBiometric.set(${JSON.stringify(state)})
  document.querySelector('#app').__vue_app__._context.provides.VUE_QUERY_CLIENT.invalidateQueries()
  return true`

const lock = async (app: App) => {
  await app.page.eval(`${VAULT} await vault.lock(); return true`)
  await app.waitFor(`return document.querySelector('${TITLE}') !== null`, 'the unlock screen')
  await app.waitFor(`return document.querySelector('${FORM}') !== null`, 'the unlock form of the last vault')
}

for (const width of [1280, 390] as const) {
  for (const theme of ['dark', 'light'] as const) {
    test(`unlock screen, ${width}px ${theme}`, async ({ app }) => {
      await app.unlock()
      await app.at('/', { width, theme })
      await app.page.eval(setDevice('enabled'))
      await lock(app)

      // Display type on the title, the canvas colour under it, the vaults as rows.
      expect(
        await app.page.eval<number>(`return getComputedStyle(document.querySelector('${TITLE}')).fontWeight|0`),
      ).toBe(500)
      // The canvas colour of the theme (pre-paint, then the token) is under the title: dark is dark, light is light.
      const canvas = await app.page.eval<number>(
        `const [red, green, blue] = getComputedStyle(document.querySelector('${TITLE}').closest('main').parentElement).backgroundColor.match(/\\d+/g).map(Number); return (red + green + blue) / 3`,
      )
      if (theme === 'dark') expect(canvas).toBeLessThan(80)
      else expect(canvas).toBeGreaterThan(200)
      expect(
        await app.page.eval<number>(
          `return document.querySelectorAll('[data-recents-row] [data-list-row-title]').length`,
        ),
      ).toBeGreaterThanOrEqual(2)
      await app.waitFor(
        `return document.querySelector('[data-biometric-unlock] button') !== null`,
        'the biometric button',
      )
      await app.screenshot(`unlock-${width}-${theme}-1-card`)
      expect(
        await app.page.eval<boolean>(
          `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`,
        ),
      ).toBe(true)

      // A wrong password: the reason in an alert, still on the screen.
      await app.type('#unlock-card-password', 'nope')
      await app.click(`${FORM} button[type="submit"]`)
      await app.waitFor(`return document.querySelector('[data-unlock-error]') !== null`, 'the error')
      await app.screenshot(`unlock-${width}-${theme}-2-error`)

      // Biometrics open the vault.
      await app.click('[data-biometric-unlock] button')
      await app.waitFor(`${VAULT} return vault.locked === false`, 'the biometric unlock')
    })
  }
}

test('the right password opens the home screen', async ({ app }) => {
  await app.unlock()
  await app.page.eval(setDevice('unavailable'))
  await lock(app)
  expect(await app.exists('[data-biometric-unlock]')).toBe(false)
  await app.type('#unlock-card-password', 'secret')
  await app.click(`${FORM} button[type="submit"]`)
  await app.waitFor(`${VAULT} return vault.locked === false`, 'the vault to open')
  await app.waitFor(`return document.querySelector('[data-tour="quick-add"]') !== null`, 'the shell')
})
