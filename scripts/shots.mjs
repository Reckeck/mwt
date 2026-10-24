// Screenshots for checking a screen by eye: every state of every screen in a set,
// at desktop and phone widths, in both themes, on the mock scenarios
// (src/shared/api/mock/scenario.ts). Not a gate — check-layout is the gate.
//
//   node scripts/shots.mjs <set> [--width 1280,390] [--theme dark] [--only <shot,…>] [--canvas]
//   mise run shots -- <set>
//
// Sets live in scripts/shots/sets.mjs. A shot with `canvas` is also put side by side
// with that frame of the design canvas: the rendered Full-*.dc.html.html boards in
// CANVAS_DIR (default .cache/canvas).
// `--canvas` renders only the canvas frames: the target, before the screen is built.
// Out: .cache/shots/<set>/<shot>-<scenario>-<theme>-<width>.png, pairs in .../pairs/.
import { existsSync, mkdirSync, rmSync } from 'node:fs'
import { join } from 'node:path'

import { findChromium, launch } from './check-layout/cdp.mjs'
import { login, mounted, setTheme } from './check-layout/scenario.mjs'
import { freePort, startVite } from './shots/serve.mjs'
import { canvasShot, ensureReady, freshStart, pairShot, runSteps, settle } from './shots/steps.mjs'
import { SETS } from './shots/sets.mjs'

const root = new URL('..', import.meta.url).pathname
const args = process.argv.slice(2)
const option = (name) => {
  const at = args.indexOf(`--${name}`)
  return at >= 0 ? args[at + 1] : null
}
const setName = args.find((arg, at) => !arg.startsWith('--') && !args[at - 1]?.startsWith('--'))
const set = SETS[setName]
if (!set) {
  console.error(`shots: unknown set '${setName ?? ''}'; sets: ${Object.keys(SETS).join(', ')}`)
  process.exit(1)
}

/** Desktop sizes (mouse), then phones (touch, with system bars). */
const DESKTOP = [[1920, 1080], [1440, 900], [1280, 800], [1100, 800], [1024, 640], [768, 800]]
const PHONE = [[390, 844], [360, 740]]
const widths = option('width')?.split(',').map(Number)
const pick = (sizes) => sizes.filter(([width]) => !widths || widths.includes(width))
const themes = option('theme') ? [option('theme')] : ['light', 'dark']
const only = option('only')
const canvasOnly = args.includes('--canvas')
const canvasDir = process.env.CANVAS_DIR ?? join(root, '.cache/canvas')

const out = join(root, '.cache/shots', setName)
// A partial run (`--only`, `--width`, `--theme`) keeps the rest of the set.
if (!only && !widths && !option('theme') && !canvasOnly) rmSync(out, { recursive: true, force: true })
mkdirSync(join(out, 'pairs'), { recursive: true })

const binary = findChromium()
if (!binary) {
  console.error('shots: no Chromium (set CHROMIUM=/path/to/chromium)')
  process.exit(1)
}

const shots = set.filter((shot) => !only || only.split(',').includes(shot.name))
const scenarios = canvasOnly ? [] : [...new Set(shots.flatMap((shot) => shot.scenarios ?? ['year']))]
const { vite, url } = await startVite(root)
const page = await launch(binary, await freePort())
const written = []
const failed = []

try {
  // Touch emulation cannot be undone: every desktop size first, in a fresh login per scenario.
  for (const touch of [false, true]) {
    const sizes = pick(touch ? PHONE : DESKTOP)
    if (!sizes.length) continue
    for (const scenario of scenarios) {
      const inScenario = shots.filter((shot) => (shot.scenarios ?? ['year']).includes(scenario))
      const fits = (shot) => !(shot.phone === false && touch) && !(shot.desktop === false && !touch)
      // `login: false` shots start from a fresh, locked app (the unlock screen); the rest share one login.
      for (const fresh of [false, true]) {
        const list = inScenario.filter((shot) => fits(shot) && (shot.login === false) === fresh)
        if (!list.length) continue
        await page.viewport(...sizes[0], touch)
        await page.goto(`${url}?mock=${scenario}`)
        if (fresh) await mounted(page)
        else await login(page)
        for (const theme of themes) {
          await setTheme(page, theme)
          for (const [width, height] of sizes) {
            await page.viewport(width, height, touch)
            await page.eval(`document.documentElement.style.cssText = '${touch ? '--native-inset-top:47px;--native-inset-bottom:34px' : ''}'; return true`)
            for (const shot of list) {
              const file = join(out, `${shot.name}-${scenario}-${theme}-${width}.png`)
              try {
                if (fresh) await freshStart(page, `${url}?mock=${scenario}`, theme, touch)
                if (!fresh) await ensureReady(page, theme, touch)
                await runSteps(page, shot, touch)
                await settle(page, scenario)
                await page.shot(file)
                written.push(file)
              } catch (error) {
                failed.push(`${shot.name} ${scenario} ${theme} ${width}: ${error.message}`)
              }
            }
          }
        }
      }
    }
  }

  // The design canvas next to the app, at the canvas frame's own size.
  if (canvasDir && existsSync(canvasDir)) {
    for (const shot of shots.filter((item) => item.canvas)) {
      for (const theme of themes) {
        for (const [kind, width] of [['d', 1280], ['p', 390]]) {
          const app = join(out, `${shot.name}-${(shot.scenarios ?? ['year'])[0]}-${theme}-${width}.png`)
          if (!canvasOnly && !existsSync(app)) continue
          const frame = join(out, 'pairs', `${shot.name}-canvas-${theme}-${width}.png`)
          const ok = await canvasShot(page, canvasDir, shot.canvas, theme, kind, frame)
          if (!ok) {
            failed.push(`${shot.name}: canvas frame '${shot.canvas.board} ${shot.canvas.frame}' (${kind}) not found`)
            continue
          }
          if (canvasOnly) {
            written.push(frame)
            continue
          }
          const pair = join(out, 'pairs', `${shot.name}-${theme}-${width}.png`)
          await pairShot(page, frame, app, pair)
          written.push(pair)
        }
      }
    }
  } else if (shots.some((shot) => shot.canvas)) {
    console.log('shots: CANVAS_DIR not set, no canvas pairs')
  }
} finally {
  await page.close()
  vite.kill()
}

for (const problem of failed) console.error(`shots: ${problem}`)
console.log(`shots: ${written.length} images in .cache/shots/${setName}/ (${failed.length} failed)`)
process.exit(failed.length ? 1 : 0)
