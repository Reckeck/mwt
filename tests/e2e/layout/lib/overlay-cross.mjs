// Layer over layer (docs/ui-overlays.md §4, items 2, 4): Ctrl K over a dialog. Each closes one layer per Esc / Back.
// The toast cases (over a sheet or a glance) return with their screens (W4).
import { FIND, LAYER, closeLayer, openLayer, press, step } from './overlay-steps.mjs'

const GOAL_CARD = '[data-goal-card]'

/** The topmost element at the middle of the layer's `inner` (or of the layer itself) is inside the layer. */
const ON_TOP = (selector, inner) => `${FIND(selector)}
  if (!layer) return false
  const target = ${inner ? `layer.querySelector(${JSON.stringify(inner)})` : 'layer'}
  if (!target) return false
  const rect = target.getBoundingClientRect()
  const at = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2)
  return Boolean(at && layer.contains(at))`

async function waitFor(page, selector) {
  for (let tries = 0; tries < 40; tries++) {
    if (await page.eval(LAYER(selector))) return true
    await page.sleep(50)
  }
  return false
}

/**
 * Item 2: Ctrl K over an open converter dialog. Either the palette shows on top, takes the focus and Esc closes
 * only it, or it waits (a modal layer holds the keys): then the dialog keeps the focus. Never a palette under it.
 */
async function paletteOverDialog(page, touch) {
  const problems = []
  const palette = '[data-command-palette][data-open="true"]'
  const dialog = '[data-overlay="converter"]'
  await openLayer(page, { route: '/' })
  await page.eval(`const converter = await import('/src/shared/lib/converter.ts'); converter.openConverter(); return true`)
  if (!(await waitFor(page, dialog))) return ['the converter did not open']
  await page.sleep(500)
  await press(page, 'k', 2)
  await page.sleep(500)
  if (await page.eval(LAYER(palette))) {
    if (!(await page.eval(ON_TOP(palette, 'input')))) problems.push('the palette is under the dialog (a click would not reach it)')
    if (!(await page.eval(`return Boolean(document.activeElement?.closest('[data-command-palette]'))`))) problems.push('focus stays in the dialog under the palette')
    if (!(await closeLayer(page, palette, touch))) problems.push('the palette over a dialog does not close')
    if (!(await page.eval(LAYER(dialog)))) problems.push('one Esc / Back closed the palette and the dialog under it')
  } else if (!(await page.eval(`${FIND(dialog)} return Boolean(layer?.contains(document.activeElement))`))) {
    problems.push('Ctrl K lost the focus of the dialog')
  }
  await closeLayer(page, dialog, touch)
  return problems
}

/** Item 6: the delete question over a goal's card; one Esc / Back closes the question, the next the card. */
async function dialogInSheet(page, touch) {
  const problems = []
  await openLayer(page, { route: '/money/plan' })
  await step(page, { query: { open: 'goal:{goal}' } })
  if (!(await waitFor(page, GOAL_CARD))) return ['the goal card did not open']
  await page.sleep(500)
  if ((await step(page, { click: '[data-goal-delete]' })) || !(await waitFor(page, '[data-overlay="goal-delete"]'))) return ['the delete question did not open over the card']
  await page.sleep(500)
  if (!(await page.eval(ON_TOP('[data-overlay="goal-delete"]')))) problems.push('the delete question is under the card')
  if (!(await closeLayer(page, '[data-overlay="goal-delete"]', touch))) problems.push('the question over the card does not close')
  // Its slide-out ends before the next Back means the card.
  for (let tries = 0; tries < 40 && (await page.eval(`return Boolean(document.querySelector('[data-overlay="goal-delete"]'))`)); tries++) await page.sleep(50)
  if (!(await page.eval(LAYER(GOAL_CARD)))) problems.push('the card closed together with the question over it')
  if (!(await closeLayer(page, GOAL_CARD, touch))) problems.push('the card does not close after the question')
  return problems
}

const CROSS = [
  { id: 'dialog-in-sheet', run: dialogInSheet },
  { id: 'palette-over-dialog', run: paletteOverDialog },
]

export const CROSS_IDS = CROSS.map((entry) => entry.id)

/** All layer-over-layer cases at the current window size. */
export async function crossLayers(page, { touch, label, only }) {
  const problems = []
  for (const entry of CROSS) {
    if (only?.length && !only.includes(entry.id)) continue
    try {
      for (const problem of await entry.run(page, touch)) problems.push(`${label} ${entry.id}: ${problem}`)
    } catch (error) {
      problems.push(`${label} ${entry.id}: ${String(error.message ?? error).split('\n')[0]}`)
    }
    await press(page, 'Escape')
    await page.sleep(150)
  }
  return problems
}
