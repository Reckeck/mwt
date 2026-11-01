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
//   mise run check-layout -- --jobs 4      # N processes on one Vite, a Chromium each   (E2E_SHARD=i/N)
//   mise run check-layout -- --shard 2/6   # the 2nd of 6 shares of the units (CI matrix) (E2E_SHARD_CI=K/N)
//   mise run check-layout -- --update-weights   # rewrite layout/weights.json from this run's timings
// The work is cut into units (tests/e2e/harness/shard.ts): `dark-1280 #2` is 5 views of one theme x width,
// `overlays-390 #1` is 10 layers. Mouse units run before touch ones in every process.
// Screenshots: .cache/check-layout/<theme>-<width>-<view>.png
import { mkdirSync } from 'node:fs'
import { join } from 'node:path'

import { afterAll, beforeAll, describe, inject, it } from 'vitest'

import { connect } from '../harness/app'
import type { Page } from '../harness/cdp.mjs'
import { flushTimings, shardOwner, timed } from '../harness/shard'
import { overlayIds, overlays } from './lib/overlays.mjs'
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

// Unit sizes. A sweep unit is a theme x width cut into chunks of about VIEWS_PER_CHUNK views; the number of
// chunks is fixed here (the unit names are the keys of weights.json) and the views are divided evenly between
// them when the unit runs, so a new view makes a chunk bigger, never lost. Counts come from scenario.mjs
// (`views()`): 8 main screens at the sizes that are not in FULL_AT (and in the light theme), all the rest at FULL_AT.
const VIEWS_PER_CHUNK = 5
const MAIN_VIEWS = 8
const FULL_VIEWS = 50
const FULL_WIDTHS = new Set([1280, 1024, 768, 560, 390, 360])
const OVERLAYS_PER_CHUNK = 10
const viewChunks = (theme: string, width: number) =>
  Math.ceil((theme === 'dark' && FULL_WIDTHS.has(width) ? FULL_VIEWS : MAIN_VIEWS) / VIEWS_PER_CHUNK)

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
    // Several processes warm one Vite at once, so a reload can cut in more than once: try each view again.
    for (const view of await views(page, '1280x720')) {
      for (let attempt = 1; ; attempt++) {
        try {
          await open(page, view)
          break
        } catch (error) {
          if (attempt === 3) throw error
          await page.sleep(1000)
          await login(page)
        }
      }
    }
  }, 300_000)

  afterAll(async () => {
    flushTimings('layout')
    await app?.close()
  })

  /** The viewport a once-per-run test needs, set only when it is not there yet (a resize is a layout of its own). */
  const sized = async (width: number, height: number, touch: boolean) => {
    if ((await page.eval('return innerWidth')) !== width) await page.viewport(width, height, touch)
  }

  const desktop = DESKTOP.filter(([width]: number[]) => !quick || width === 1280)
  const phone = PHONE.filter(([width]: number[]) => !quick || width === 390)

  // ---- the units: names are the keys of weights.json, so they stay as they are ----
  type Sweep = { theme: string; width: number; height: number; touch: boolean; chunk: number }
  const sweeps: Sweep[] = []
  const planSweeps = (sizes: number[][], touch: boolean) => {
    for (const theme of ['dark', 'light']) {
      for (const [width, height] of sizes) {
        if (theme === 'light' && width !== 1280 && width !== 390) continue
        const chunks = viewChunks(theme, width)
        for (let chunk = 1; chunk <= chunks; chunk++) sweeps.push({ theme, width, height, touch, chunk })
      }
    }
  }
  planSweeps(desktop, false)
  planSweeps(phone, true)
  const sweepName = ({ theme, width, chunk }: Sweep) => `${theme}-${width} #${chunk}`

  const overlayUnits = (touch: boolean) => {
    const label = touch ? '390' : '1280'
    const ids = [...new Set<string>(overlayIds(touch))]
    const count = Math.ceil(ids.length / OVERLAYS_PER_CHUNK)
    const size = Math.ceil(ids.length / count)
    return Array.from({ length: count }, (_unused, at) => ({
      name: `overlays-${label} #${at + 1}`,
      label,
      ids: ids.slice(at * size, (at + 1) * size),
    }))
  }
  const mouseOverlays = overlayUnits(false)
  const touchOverlays = overlayUnits(true)

  const ONCE_MOUSE = [
    'dialog opens in place',
    'no listener leaks',
    'confirm button is stable',
    'toast over a dialog can be clicked',
  ]
  const ONCE_TOUCH = ['Back closes layers']
  const mine = shardOwner('layout', [
    ...sweeps.map(sweepName),
    ...ONCE_MOUSE,
    ...mouseOverlays.map((unit) => unit.name),
    ...ONCE_TOUCH,
    ...touchOverlays.map((unit) => unit.name),
  ])

  /** One chunk of the views of a theme x width; the viewport and theme are set here, not left by a neighbour. */
  const runSweep = (unit: Sweep) =>
    it.skipIf(!mine(sweepName(unit)) || onlyOverlays)(
      sweepName(unit),
      () =>
        timed(sweepName(unit), async () => {
          const { theme, width, height, touch, chunk } = unit
          await setTheme(page, theme)
          await page.viewport(width, height, touch)
          const size = `${width}x${height}`
          const all: string[] = await views(page, size)
          // Light theme: colours only change, so the main screens are enough.
          const list = theme === 'light' ? all.slice(0, 8) : all
          const chunks = viewChunks(theme, width)
          const per = Math.ceil(list.length / chunks)
          const problems: string[] = []
          for (const view of list.slice((chunk - 1) * per, chunk * per)) {
            await open(page, view)
            const name = view.replace(/[^\w-]+/g, '_').replace(/^_|_$/g, '') || 'dashboard'
            await page.shot(join(SHOTS, `${theme}-${width}-${name}.png`))
            let found: string[] = await measureView(page, touch)
            // A loaded machine (--jobs, a shared runner) can stall the browser; a stall of the app's own comes back
            // on a second look. A layout shift gets no second look: late data moving the page is what rule 17 forbids.
            if (found.some((item) => item.startsWith('long task'))) {
              await open(page, view)
              found = await measureView(page, touch)
            }
            for (const item of found) problems.push(`${theme} ${size} ${view}: ${item}`)
          }
          await setTheme(page, 'dark')
          settle('views', problems)
        }),
      240_000,
    )

  const runOnce = (name: string, work: () => Promise<void>) =>
    it.skipIf(!mine(name))(name, () => timed(name, work), 120_000)

  const runOverlays = (unit: { name: string; label: string; ids: string[] }, touch: boolean) => {
    // --overlays=id narrows every unit to its own ids; a unit with none of them has nothing to do.
    const ids = onlyOverlays ? unit.ids.filter((id) => only.includes(id)) : unit.ids
    it.skipIf(!mine(unit.name) || !ids.length)(
      unit.name,
      () =>
        timed(unit.name, async () => {
          await sized(touch ? 390 : 1280, touch ? 844 : 720, touch)
          settle('overlays', await overlays(page, { touch, label: unit.label, only: ids }))
        }),
      240_000,
    )
  }

  // Mouse first: touch emulation leaves the pointer coarse for the rest of the run.
  describe('desktop views (mouse)', () => {
    for (const unit of sweeps.filter((item) => !item.touch)) runSweep(unit)
  })

  // Each of these sets the viewport it needs: a shard may not run the test that used to leave it right.
  describe('once per run, mouse', () => {
    runOnce('dialog opens in place', async () => {
      await page.viewport(1280, 720, false)
      if (!onlyOverlays) settle('dialog', await dialogJump(page))
    })
    runOnce('no listener leaks', async () => {
      await sized(1280, 720, false)
      if (!onlyOverlays) settle('listeners', await listenerLeak(page))
    })
    runOnce('confirm button is stable', async () => {
      await sized(1280, 720, false)
      if (!onlyOverlays) settle('confirm', await confirmStable(page))
    })
    runOnce('toast over a dialog can be clicked', async () => {
      await sized(1280, 720, false)
      if (!onlyOverlays) settle('toast', await toastOverDialog(page))
    })
    for (const unit of mouseOverlays) runOverlays(unit, false)
  })

  describe('phone views (touch)', () => {
    for (const unit of sweeps.filter((item) => item.touch)) runSweep(unit)
  })

  describe('once per run, touch', () => {
    runOnce('Back closes layers', async () => {
      await page.viewport(390, 844, true)
      if (!onlyOverlays) settle('back (phone)', await backCloses(page))
    })
    for (const unit of touchOverlays) runOverlays(unit, true)
  })
})
