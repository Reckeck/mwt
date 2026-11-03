import { expect, test } from '../harness/app'

const SETTINGS = '/?settings=app&section=updates'
const NOTICE = '[data-testid="update-notice"]'

/** The mock cycles up to date, available, ready on every check (src/shared/api/mock/commands-updates.ts). */
for (const [width, theme] of [
  [1280, 'dark'],
  [1280, 'light'],
  [390, 'dark'],
  [390, 'light'],
] as const) {
  test(`updates section and notice, ${width}px ${theme}`, async ({ app }) => {
    await app.unlock()
    await app.at(SETTINGS, { width, theme })
    await app.waitFor(`return document.querySelector('[data-tour="settings-updates"]') !== null`, 'the updates section')
    const geometry = `const box = document.querySelector('[data-tour="settings-updates"]').getBoundingClientRect(); return Math.round(box.height)`
    // The notice sits under the settings dialog: close it for a shot of the notice alone, then reopen.
    const noticeShot = async (name: string) => {
      await app.press('Escape')
      await app.waitFor(`return document.querySelector('[role="dialog"]') === null`, 'the settings to close')
      await app.screenshot(name)
      await app.at(SETTINGS, { width, theme })
      await app.waitFor(
        `return document.querySelector('[data-tour="settings-updates"]') !== null`,
        'the updates section',
      )
    }
    const noOverflow = `return document.documentElement.scrollWidth <= document.documentElement.clientWidth`
    await app.screenshot(`app-update-${width}-${theme}-1-initial`)

    // Step the mock to up to date first (it cycles on every check and keeps state between tests).
    for (let attempt = 0; attempt < 3; attempt++) {
      if (/is up to date/i.test(await app.text('[data-tour="settings-updates"]'))) break
      await app.clickText('button', '^Check now$')
      await app.page.eval(`await new Promise((done) => setTimeout(done, 300)); return true`)
    }
    // Check until the notice shows "available" (the mock steps once per check).
    await app.clickText('button', '^Check now$')
    await app.waitFor(`return /Version 9\\.9\\.9 is available/.test(document.body.textContent)`, 'available')
    await app.waitFor(`return document.querySelector('${NOTICE}') !== null`, 'the notice')
    const heightAvailable = await app.page.eval<number>(geometry)
    await app.screenshot(`app-update-${width}-${theme}-2-available`)
    await noticeShot(`app-update-${width}-${theme}-2-notice-available`)
    await app.waitFor(noOverflow, 'no horizontal scroll')

    await app.clickText('button', '^Check now$')
    await app.waitFor(`return /Version 9\\.9\\.9 is ready/.test(document.body.textContent)`, 'ready')
    await app.waitFor(
      `return /is ready/.test(document.querySelector('${NOTICE}')?.textContent ?? '')`,
      'the ready notice',
    )
    const heightReady = await app.page.eval<number>(geometry)
    await app.screenshot(`app-update-${width}-${theme}-3-ready`)
    await noticeShot(`app-update-${width}-${theme}-3-notice-ready`)
    await app.waitFor(noOverflow, 'no horizontal scroll')
    expect(heightReady).toBe(heightAvailable)

    // Touch targets of the notice buttons.
    const small = await app.page.eval<number>(
      `return [...document.querySelectorAll('${NOTICE} button')].filter((el) => el.getBoundingClientRect().height < ${width === 390 ? 40 : 28}).length`,
    )
    expect(small).toBe(0)

    await app.clickText(`${NOTICE} button`, '^Later$')
    await app.waitFor(`return document.querySelector('${NOTICE}') === null`, 'the notice to hide')
  })
}
