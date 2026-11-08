import { expect, test } from '../harness/app'

/**
 * Sync refusal "level 3 required" on the mock (Sync refusal 1): the device below level 3 shows a banner on the
 * Devices tab, the sender marks the row of the peer that refused. Both get the same `sync-error` event; the mock
 * level decides which side this device is.
 */
const DEVICES = '/?settings=sync&section=sync-devices'
const BANNER = '[data-sync-level-required]'
const PEER_NOTE = '[data-peer-level-required]'

/** Mock state shared by the steps below: this device's level and sync on/off, plus an optional refusal event. */
const arrange = (level: string | null, syncOn: boolean, refuse: boolean) => `
  // The module instances the app loaded (Vite may have added a ?t= stamp to their URLs).
  const urlOf = (name) => performance.getEntriesByType('resource').map((entry) => entry.name).reverse().find((url) => url.includes(name)) ?? '/src/' + name
  const security = await import(urlOf('mock/commands-security-state.ts'))
  const data = await import(urlOf('mock/vault-data.ts'))
  const refusal = await import('/src/shared/api/mock/sync-refusal.ts')
  const model = await import(urlOf('features/vault-sync/model/use-sync-level-refusal.ts'))
  const queryClient = document.querySelector('#app').__vue_app__._context.provides.VUE_QUERY_CLIENT
  data.mockPassword.value = 'secret'
  security.mockSecurityLevel.value = ${JSON.stringify(level)}
  data.mockAppSettings.sync.enabled = ${syncOn}
  model.useSyncLevelRefusal().reset()
  await queryClient.resetQueries()
  ${refuse ? `refusal.emitMockLevelRefusal(${JSON.stringify('Ноутбук')})` : ''}
  return true`

const noOverflow = `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`

for (const [width, theme] of [
  [1280, 'light'],
  [1280, 'dark'],
  [390, 'light'],
  [390, 'dark'],
] as const) {
  test(`level 3 refusal: banner and sender note, ${width}px ${theme}`, async ({ app }) => {
    await app.unlock()
    // The first dynamic import of a mock module may make Vite reload the page: do it before the scenario starts.
    await app.page.eval(arrange(null, false, false)).catch(() => undefined)
    await app.unlock()
    const prefix = `sync-level-${width}-${theme}`

    // Receiver: below level 3, the refusal shows the banner and the button opens the wizard.
    await app.at(DEVICES, { width, theme })
    await app.page.eval(arrange(null, true, false))
    await app.waitFor(`return document.querySelector('[data-tour="sync-device-list"]') !== null`, 'the device list')
    expect(await app.page.eval<boolean>(`return document.querySelector('${BANNER}') === null`)).toBe(true)
    await app.page.eval(arrange(null, true, true))
    await app.waitFor(`return document.querySelector('${BANNER}') !== null`, 'the banner')
    expect(await app.text(BANNER)).toMatch(/Crypto data was not accepted on this device/)
    expect(await app.page.eval<boolean>(`return document.querySelector('${PEER_NOTE}') === null`)).toBe(true)
    await app.waitFor(noOverflow, 'no horizontal scroll')
    await app.screenshot(`${prefix}-1-banner`)
    await app.click(`${BANNER} [data-tour="security-raise-start"]`)
    await app.waitFor(`return document.querySelector('[data-raise-wizard]') !== null`, 'the wizard')
    await app.press('Escape')
    await app.waitFor(`return document.querySelector('[data-raise-stop]') !== null`, 'the interruption question')
    await app.click('[data-raise-stop]')
    await app.waitFor(`return document.querySelector('[data-raise-wizard]') === null`, 'the wizard to close')

    // Level 3 reached: the banner goes.
    await app.page.eval(arrange('crypto', true, false))
    await app.page.eval(
      `document.querySelector('#app').__vue_app__._context.provides.VUE_QUERY_CLIENT.invalidateQueries(); return true`,
    )
    await app.waitFor(`return document.querySelector('${BANNER}') === null`, 'the banner to go')

    // Sender: at level 3, the peer that refused gets the note on its row.
    await app.page.eval(arrange('crypto', true, true))
    await app.waitFor(`return document.querySelector('${PEER_NOTE}') !== null`, 'the note on the peer row')
    expect(await app.text(PEER_NOTE)).toMatch(/is still below protection level 3/)
    expect(await app.page.eval<boolean>(`return document.querySelector('${BANNER}') === null`)).toBe(true)
    await app.waitFor(noOverflow, 'no horizontal scroll')
    await app.screenshot(`${prefix}-2-peer-note`)

    // Leave the mock as the other files expect it.
    await app.page.eval(arrange(null, false, false))
  })
}
