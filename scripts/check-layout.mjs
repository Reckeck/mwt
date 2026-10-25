// The behaviour gate (AGENTS.md rule 17): the interface does not move, overflow
// or hide its actions by itself. Static rules live in check-ui; this one runs
// the app (`vite` + the in-memory mock) in headless Chromium and measures it.
//
// Per view, at 1920…720 and 560 (a tiled window; mouse: pointer fine + hover, as WebKitGTK reports)
// and 390/360 (touch), dark theme everywhere and light on the main screens:
// - the page never scrolls sideways; no element is wider than a box that does not scroll;
// - layout shift after opening a view stays under 0.01; no long task over 500 ms;
// - mouse: hovering a list row and leaving moves nothing;
// - touch: controls are 40×40 (or have that hit area) and none is invisible.
// Once per run: a dialog opens in place, a second visit adds no event listener,
// Back closes the settings window, the quick-add dialog and the command palette,
// an armed ConfirmButton keeps its width and focus, and a toast over a dialog
// can be clicked without closing the dialog.
//
//   node scripts/check-layout.mjs            # gate: exit 1 on a problem
//   node scripts/check-layout.mjs --report   # print only
//   node scripts/check-layout.mjs --quick    # 1280 and 390 only
// Screenshots: .cache/check-layout/<theme>-<width>-<view>.png
import { mkdirSync } from 'node:fs'
import { join } from 'node:path'

import { findChromium, launch } from './check-layout/cdp.mjs'
import {
  DESKTOP,
  PHONE,
  backCloses,
  confirmStable,
  dialogJump,
  listenerLeak,
  login,
  measureView,
  open,
  setTheme,
  toastOverDialog,
  views,
} from './check-layout/scenario.mjs'
import { freePort, startVite } from './check-layout/server.mjs'

const root = new URL('..', import.meta.url).pathname
const report = process.argv.includes('--report')
const quick = process.argv.includes('--quick')
const shots = join(root, '.cache/check-layout')
mkdirSync(shots, { recursive: true })

const binary = findChromium()
if (!binary) {
  console.error('check-layout: no Chromium (set CHROMIUM=/path/to/chromium)')
  process.exit(report ? 0 : 1)
}

const { vite, url } = await startVite()
const page = await launch(binary, await freePort())
const problems = []
const add = (where, list) => {
  for (const item of list) problems.push(`${where}: ${item}`)
}

try {
  const desktop = DESKTOP.filter(([width]) => !quick || width === 1280)
  const phone = PHONE.filter(([width]) => !quick || width === 390)
  await page.viewport(1280, 720, false)
  await page.goto(url)
  await login(page)

  const sweep = async (sizes, touch) => {
    for (const theme of ['dark', 'light']) {
      await setTheme(page, theme)
      for (const [width, height] of sizes) {
        // Light theme: colours only change, so the main sizes are enough.
        if (theme === 'light' && width !== 1280 && width !== 390) continue
        await page.viewport(width, height, touch)
        const size = `${width}x${height}`
        const list = await views(page, size)
        for (const view of theme === 'light' ? list.slice(0, 8) : list) {
          await open(page, view)
          const name = view.replace(/[/:]+/g, '_').replace(/^_|_$/g, '') || 'dashboard'
          await page.shot(join(shots, `${theme}-${width}-${name}.png`))
          add(`${theme} ${size} ${view}`, await measureView(page, touch))
        }
      }
    }
    await setTheme(page, 'dark')
  }

  // Mouse first: touch emulation leaves the pointer coarse for the rest of the run.
  await sweep(desktop, false)
  await page.viewport(1280, 720, false)
  add('dialog', await dialogJump(page))
  add('listeners', await listenerLeak(page))
  add('confirm', await confirmStable(page))
  add('toast', await toastOverDialog(page))
  await sweep(phone, true)
  await page.viewport(390, 844, true)
  add('back (phone)', await backCloses(page))
} finally {
  await page.close()
  vite.kill()
}

for (const problem of problems) console.error(`${report ? 'note' : 'error'}: ${problem}`)
console.log(`check-layout: ${problems.length} problems; screenshots in .cache/check-layout/`)
process.exit(problems.length && !report ? 1 : 0)
