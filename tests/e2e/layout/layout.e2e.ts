// The behaviour gate (AGENTS.md rule 17): the interface does not move, overflow or hide its actions by
// itself. Static rules live in tests/checks/ui; this runs the app (Vite + the in-memory mock) in headless
// Chromium and measures it.
//
// Per view, at 1920...720 and 560 (mouse: pointer fine + hover, as WebKitGTK reports) and 390/360 (touch),
// dark theme everywhere and light on the main screens: no sideways scroll, no element wider than a box that
// does not scroll, layout shift < 0.01, no long task over 500 ms, hovering a row moves nothing (mouse),
// touch targets are 40x40 (touch). Once per run: dialog opens in place, no listener leak, Back closes layers,
// every layer of overlay-list.mjs, the confirm button, a toast over a dialog.
//
//   mise run check-layout                  # gate
//   mise run check-layout -- --quick       # 1280 and 390 only        (E2E_QUICK=1)
//   mise run check-layout -- --report      # print only, never fail   (E2E_REPORT=1)
//   mise run check-layout -- --overlays=id,id   # only those layers   (E2E_OVERLAYS)
// Screenshots: .cache/check-layout/<theme>-<width>-<view>.png
import { mkdirSync } from 'node:fs'
import { join } from 'node:path'

import { afterAll, beforeAll, describe, inject, it } from 'vitest'

import { connect } from '../harness/app'
import type { Page } from '../harness/cdp.mjs'
import { overlays } from './lib/overlays.mjs'
import {
  backCloses,
  confirmStable,
  DESKTOP,
  dialogJump,
  listenerLeak,
  login,
  measureView,
  open,
  PHONE,
  setTheme,
  toastOverDialog,
  views,
} from './lib/scenario.mjs'

const ROOT = new URL('../../..', import.meta.url).pathname
const SHOTS = join(ROOT, '.cache/check-layout')
const quick = process.env.E2E_QUICK === '1'
const report = process.env.E2E_REPORT === '1'
const only = (process.env.E2E_OVERLAYS ?? '').split(',').filter(Boolean)
const onlyOverlays = process.env.E2E_OVERLAYS !== undefined

/** Problems are the test's failure; --report prints them instead. */
function settle(where: string, problems: string[]) {
  if (!problems.length) return
  const lines = problems.map((problem) => `${where}: ${problem}`)
  if (report) console.error(lines.map((line) => `note: ${line}`).join('\n'))
  else throw new Error(`\n${lines.map((line) => `error: ${line}`).join('\n')}`)
}

describe('check-layout', () => {
  let app: Awaited<ReturnType<typeof connect>>
  let page: Page
  let url = ''

  beforeAll(async () => {
    mkdirSync(SHOTS, { recursive: true })
    app = await connect({ animations: true })
    page = app.page
    // `inject('e2e')` is read by connect; the same url is what unlock() navigates to.
    url = inject('e2e').url
    await page.viewport(1280, 720, false)
    await page.goto(url)
    await login(page)
    // On a clean checkout the first visit to a route is Vite compiling it, and may reload the page: not the app.
    for (const view of await views(page, '1280x720')) await open(page, view)
  }, 300_000)

  afterAll(async () => {
    await app?.close()
  })

  const desktop = DESKTOP.filter(([width]: number[]) => !quick || width === 1280)
  const phone = PHONE.filter(([width]: number[]) => !quick || width === 390)

  // Mouse first: touch emulation leaves the pointer coarse for the rest of the run.
  const sweep = (label: string, sizes: number[][], touch: boolean) =>
    describe.each(['dark', 'light'])(`${label} (%s theme)`, (theme) => {
      // Light theme: colours only change, so the main sizes are enough (an empty suite is an error in vitest).
      const themed = theme === 'light' ? sizes.filter(([width]) => width === 1280 || width === 390) : sizes
      if (!themed.length) return
      describe.each(themed)('%i x %i', (width, height) => {
        it('every view is stable', async () => {
          await setTheme(page, theme)
          await page.viewport(width, height, touch)
          const size = `${width}x${height}`
          const list: string[] = await views(page, size)
          const problems: string[] = []
          for (const view of theme === 'light' ? list.slice(0, 8) : list) {
            await open(page, view)
            const name = view.replace(/[^\w-]+/g, '_').replace(/^_|_$/g, '') || 'dashboard'
            await page.shot(join(SHOTS, `${theme}-${width}-${name}.png`))
            let found: string[] = await measureView(page, touch)
            // A shared runner can stall the browser once; the app's own long task comes back on a second look.
            if (found.some((item) => item.startsWith('long task'))) {
              await open(page, view)
              found = await measureView(page, touch)
            }
            for (const item of found) problems.push(`${theme} ${size} ${view}: ${item}`)
          }
          await setTheme(page, 'dark')
          settle('views', problems)
        }, 900_000)
      })
    })

  if (!onlyOverlays) sweep('desktop', desktop, false)

  describe('once per run, mouse', () => {
    it('dialog opens in place', async () => {
      await page.viewport(1280, 720, false)
      if (!onlyOverlays) settle('dialog', await dialogJump(page))
    }, 120_000)
    it('no listener leaks', async () => {
      if (!onlyOverlays) settle('listeners', await listenerLeak(page))
    }, 120_000)
    it('confirm button is stable', async () => {
      if (!onlyOverlays) settle('confirm', await confirmStable(page))
    }, 120_000)
    it('toast over a dialog can be clicked', async () => {
      if (!onlyOverlays) settle('toast', await toastOverDialog(page))
    }, 120_000)
    it('overlays at 1280', async () => {
      settle('overlays', await overlays(page, { touch: false, label: '1280', only }))
    }, 900_000)
  })

  if (!onlyOverlays) sweep('phone', phone, true)

  describe('once per run, touch', () => {
    it('Back closes layers', async () => {
      await page.viewport(390, 844, true)
      if (!onlyOverlays) settle('back (phone)', await backCloses(page))
    }, 120_000)
    it('overlays at 390', async () => {
      settle('overlays', await overlays(page, { touch: true, label: '390', only }))
    }, 900_000)
  })
})
