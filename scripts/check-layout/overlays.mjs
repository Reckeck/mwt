// Overlays (docs/ui-overlays.md §6): every layer in overlay-list.mjs is opened, measured and closed.
// A layer is found by `data-overlay="<id>"` (the `overlay` prop of Dialog, FormDialog, SheetContent,
// PopoverContent, Glance; `layer` of DrawerContent) or by a selector the layer already had.
import { OVERLAYS } from './overlay-list.mjs'
import { APP, LAYER, FIND, PAGE, closeLayer, differs, openLayer, press, step } from './overlay-steps.mjs'
import { login } from './scenario.mjs'
import { crossLayers } from './overlay-cross.mjs'
import { OVERFLOW, TOUCH } from './probes.mjs'

export { OVERLAYS }

/** Open, measure and close one layer at the current window size. Returns problems. */
async function run(page, entry, touch) {
  const problems = []
  await openLayer(page, entry)
  const before = await page.eval(PAGE)
  for (const item of (touch && entry.phoneOpen) || entry.open) {
    const failure = await step(page, item)
    if (failure) return [failure]
  }
  // Opened: first sample as soon as it shows (the jump), then once it has settled.
  const samples = []
  for (let tries = 0; tries < 40 && !samples.length; tries++) {
    const rect = await page.eval(LAYER(entry.layer))
    if (rect) samples.push(rect)
    else await page.sleep(50)
  }
  if (!samples.length) return [`did not open: ${entry.layer}`]
  for (const wait of [40, 60, 300]) {
    await page.sleep(wait)
    const rect = await page.eval(LAYER(entry.layer))
    if (rect) samples.push(rect)
  }
  const [left, top, width, height] = samples.at(-1)
  const centre = (rect) => [rect[0] + rect[2] / 2, rect[1] + rect[3] / 2]
  const view = await page.eval(`return [innerWidth, innerHeight]`)
  if (left < -1 || top < -1 || left + width > view[0] + 1 || top + height > view[1] + 1)
    problems.push(`off screen: ${left},${top} ${width}x${height} in ${view.join('x')}`)
  if (entry.centred && !touch) {
    const [finalX, finalY] = centre(samples.at(-1))
    const worst = Math.max(...samples.map((rect) => Math.max(Math.abs(centre(rect)[0] - finalX), Math.abs(centre(rect)[1] - finalY))))
    if (worst > 2) problems.push(`moves ${Math.round(worst)}px while it opens`)
  }
  await page.sleep(200)
  const later = await page.eval(LAYER(entry.layer))
  if (later && differs(later, samples.at(-1), 2)) problems.push(`moves after it settled: ${samples.at(-1).join(',')} -> ${later.join(',')}`)

  const inside = await page.eval(`${FIND(entry.layer)}
    if (!layer) return null
    window.__scope = layer
    const has = layer.contains(document.activeElement)
    const wide = layer.scrollWidth - layer.clientWidth
    return { focus: has, wide }`)
  if (entry.focus !== false && !inside?.focus) problems.push('focus is not inside the layer')
  if (inside?.wide > 2 && entry.layer.includes('data-overlay')) problems.push(`layer content ${inside.wide}px wider than its box`)
  problems.push(...(await page.eval(OVERFLOW)))
  if (touch && entry.touch !== false) problems.push(...(await page.eval(TOUCH)))
  const during = await page.eval(PAGE)
  await page.eval(`window.__scope = undefined; return true`)

  // Close: Esc with a mouse, «Назад» on a phone.
  if (!(await closeLayer(page, entry.layer, touch))) problems.push(touch ? 'Back does not close it' : 'Esc does not close it')
  await page.sleep(200)
  for (const key of entry.keys ?? []) {
    if (await page.eval(`${APP} return gp.$route.query[${JSON.stringify(key)}] !== undefined`)) problems.push(`?${key}= stays in the address after closing`)
  }
  const after = await page.eval(PAGE)
  if (differs(before, after)) problems.push(`the page under it changed ${before.join(',')} -> ${after.join(',')}`)
  else if (differs(before, during, 20) && entry.modalShift !== true) problems.push(`the page under it changed while open ${before.join(',')} -> ${during.join(',')}`)
  return problems
}

/** One entry with its guards: a failure of the steps is a problem of that entry, not of the run. */
async function runGuarded(page, entry, touch, label) {
  const problems = []
  try {
    for (const problem of await run(page, entry, touch)) problems.push(`${label} ${entry.id}: ${problem}`)
  } catch (error) {
    problems.push(`${label} ${entry.id}: ${String(error.message ?? error).split('\n')[0]}`)
  }
  // Whatever stayed open must not leak into the next entry.
  if (entry.cleanup) await page.eval(entry.cleanup)
  await page.eval(`window.__scope = undefined; return true`)
  await press(page, 'Escape')
  await page.sleep(150)
  return problems
}

/** All layers at the current window size; `only` filters by id. The ones on the lock screen go last. */
export async function overlays(page, { touch, label, only }) {
  const problems = []
  const wanted = OVERLAYS.filter((entry) => (!only?.length || only.includes(entry.id)) && (entry.phone === 'only' ? touch : !touch || entry.phone))
  for (const entry of wanted.filter((item) => !item.locked)) problems.push(...(await runGuarded(page, entry, touch, label)))
  problems.push(...(await crossLayers(page, { touch, label, only })))
  const locked = wanted.filter((item) => item.locked)
  if (locked.length) {
    await page.eval(`${APP} await gp.$pinia._s.get('vault').lock(); return true`)
    await page.sleep(1200)
    for (const entry of locked) problems.push(...(await runGuarded(page, entry, touch, label)))
    await login(page)
  }
  return problems
}
