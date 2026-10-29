// A layer inside a layer (docs/ui-overlays.md §8): a select in a dialog, a confirm question in a sheet, the money
// card over a glance, a menu over a card, a date popover in a dialog. For each pair, mouse and touch alike:
// the child shows on top; a press inside it keeps the parent; a press on the parent (outside the child) closes
// only the child; Esc / Back takes one layer at a time, from the top; the focus goes back into the parent.
import { FIND, LAYER, closeLayer, openLayer, pointOf, press, step } from './overlay-steps.mjs'

const overlay = (id) => `[data-overlay="${id}"]`

/** Types into a field the way a keyboard does (an input event Vue and Reka see). */
const TYPE_IN = (selector, text) => `
  const field = document.querySelector(${JSON.stringify(selector)})
  field.focus()
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(field, ${JSON.stringify(text)})
  field.dispatchEvent(new Event('input', { bubbles: true }))
  return true`

export const NESTED = [
  {
    id: 'select-in-dialog',
    route: '/automations/alerts',
    open: [{ query: { new: 'alert' } }],
    parent: overlay('price-alert'),
    child: '[role="listbox"]',
    openChild: [{ click: `${overlay('price-alert')} [data-slot="select-trigger"]` }],
    inside: '[role="option"]',
    insideCloses: true,
  },
  {
    id: 'date-in-dialog',
    route: '/',
    open: [{ click: '[data-tour="quick-add"]' }],
    phoneOpen: [{ click: '[aria-label="Новая операция"]' }],
    parent: overlay('quick-add'),
    child: '[data-slot="popover-content"]',
    openChild: [
      { click: `${overlay('quick-add')} [data-slot="collapsible-trigger"]`, once: true },
      { click: `${overlay('quick-add')} [data-slot="popover-trigger"]`, text: 'Сейчас' },
    ],
  },
  {
    id: 'confirm-in-sheet',
    route: '/automations',
    open: [{ query: { open: 'automation:{automation}' } }],
    parent: '[data-automation-card]',
    child: overlay('confirm'),
    openChild: [{ click: '[data-automation-card] [data-slot="confirm-button"]' }],
  },
  {
    id: 'menu-in-sheet',
    route: '/transactions',
    open: [{ click: '[data-tx-open]' }],
    parent: '[data-tx-card]',
    child: '[role="menu"]',
    openChild: [{ click: '[data-tx-card] button', text: '^\\s*Добавить' }],
    corner: true,
    // On a phone the same button opens a sheet.
    phoneChild: '[data-attachment-sheet]',
  },
  {
    id: 'money-in-glance',
    route: '/',
    open: [{ click: '[data-glance-trigger="capital"]' }],
    parent: overlay('glance-capital'),
    child: overlay('money-card'),
    openChild: [{ click: `${overlay('glance-capital')} [data-money-convertible]` }],
  },
  {
    id: 'money-in-sheet',
    route: '/transactions',
    open: [{ click: '[data-tx-open]' }],
    parent: '[data-tx-card]',
    child: overlay('money-card'),
    openChild: [{ click: '[data-tx-card] [data-money-convertible]:not(a *, button *, [role="button"] *)' }],
  },
  {
    id: 'select-in-settings',
    route: '/?settings=accounting&section=categories',
    open: [],
    desktop: true,
    parent: overlay('settings'),
    child: '[role="listbox"]',
    openChild: [{ click: `${overlay('settings')} [data-slot="select-trigger"]` }],
    inside: '[role="option"]',
    insideCloses: true,
  },
  {
    id: 'confirm-in-settings',
    route: '/?settings=accounting&section=categories',
    open: [],
    desktop: true,
    parent: overlay('settings'),
    child: overlay('confirm'),
    openChild: [{ click: `${overlay('settings')} [data-slot="confirm-button"]` }],
  },
  {
    id: 'dialog-in-settings',
    route: '/?settings=vault&section=file',
    open: [],
    parent: overlay('settings'),
    child: overlay('password-change'),
    openChild: [{ click: 'button', text: 'Сменить пароль' }],
  },
  // A name typed into a contact field offers `Create "…"`: the contact form opens over the form, the name filled in.
  {
    id: 'contact-create-in-form',
    route: '/money/together',
    open: [{ click: '[data-tour="shared-new"]' }],
    parent: overlay('shared-event-edit'),
    child: overlay('contact-edit'),
    openChild: [
      { eval: TYPE_IN('input[aria-label^="Добавить участника"]', 'Гриша') },
      { click: '[role="option"]', text: 'Создать' },
    ],
  },
]

export const NESTED_IDS = NESTED.map((entry) => entry.id)

async function waitFor(page, selector, gone = false) {
  for (let tries = 0; tries < 40; tries++) {
    if (Boolean(await page.eval(LAYER(selector))) !== gone) return true
    await page.sleep(60)
  }
  return false
}

/** Gone from the page: a drawer slides out for half a second after it counts as closed. */
async function detached(page, selector) {
  for (let tries = 0; tries < 50; tries++) {
    if (!(await page.eval(`return Boolean(document.querySelector(${JSON.stringify(selector)}))`))) return true
    await page.sleep(60)
  }
  return false
}

/** A press like a person's: a touch on a phone, a mouse button otherwise. */
async function press_(page, touch, [x, y]) {
  if (touch) {
    await page.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] })
    await page.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] })
  } else {
    await page.mouse(x, y)
    for (const type of ['mousePressed', 'mouseReleased']) await page.send('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1 })
  }
  await page.sleep(350)
}

/** A point inside `selector`'s layer, away from `avoid`'s layer and from anything pressable. */
const SPOT = (selector, avoid, pressable) => `${FIND(selector)}
  if (!layer) return null
  const box = layer.getBoundingClientRect()
  const skip = ${avoid ? `[...document.querySelectorAll(${JSON.stringify(avoid)})].map((n) => n.getBoundingClientRect())` : '[]'}
  const near = (x, y, r) => x >= r.left - 4 && x <= r.right + 4 && y >= r.top - 4 && y <= r.bottom + 4
  for (let y = Math.max(box.top, 0) + 6; y < Math.min(box.bottom, innerHeight) - 4; y += 10) {
    for (let x = box.left + 8; x < Math.min(box.right, innerWidth) - 4; x += 12) {
      const at = document.elementFromPoint(x, y)
      if (!at || !layer.contains(at) || skip.some((r) => near(x, y, r))) continue
      if (${pressable ? 'false' : `at.closest('button, a, input, textarea, select, [role="button"], [role="option"], [role="menuitem"], [role="combobox"], [data-money-convertible], [role="tab"], label')`}) continue
      return [Math.round(x), Math.round(y)]
    }
  }
  // A modal child (a select) takes the page's pointer events: aim at a text of the parent by geometry.
  for (const node of layer.querySelectorAll('h1, h2, h3, p, label')) {
    const r = node.getBoundingClientRect()
    const x = r.left + Math.min(r.width / 2, 20), y = r.top + r.height / 2
    if (r.width && r.height && y > 0 && y < innerHeight && !skip.some((s) => near(x, y, s))) return [Math.round(x), Math.round(y)]
  }
  return null`

const FOCUS_IN = (selector) => `${FIND(selector)} return Boolean(layer?.contains(document.activeElement))`
const ON_TOP = (selector) => `${FIND(selector)}
  if (!layer) return false
  const rect = layer.getBoundingClientRect()
  const at = document.elementFromPoint(Math.min(Math.max(rect.left + rect.width / 2, 0), innerWidth - 1), Math.min(Math.max(rect.top + rect.height / 2, 0), innerHeight - 1))
  return Boolean(at && layer.contains(at))`

async function openPair(page, entry, touch) {
  await openLayer(page, entry)
  for (const item of (touch && entry.phoneOpen) || entry.open) if (await step(page, item)) return 'the parent did not open'
  if (!(await waitFor(page, entry.parent))) return 'the parent did not open'
  await page.sleep(500)
  return null
}

async function openChild(page, entry, touch, again = false) {
  // The money card ignores a press on its own sum right after it closed (that press is the one that closed it).
  await page.sleep(500)
  for (const item of entry.openChild) {
    if (again && item.once) continue
    if (item.eval) {
      await page.eval(item.eval)
      await page.sleep(item.wait ?? 400)
      continue
    }
    // A press like a person's (a touch on a phone), not a mouse click in a touch window.
    const at = await pointOf(page, item.click, item.text)
    if (!at) return false
    await press_(page, touch, at)
  }
  if (!(await waitFor(page, (touch && entry.phoneChild) || entry.child))) return false
  // A sheet slides in for half a second; a press before it settles lands on what is still moving.
  await page.sleep(800)
  return true
}

async function closeTop(page, touch) {
  if (touch) await page.eval(`const { closeTopLayer } = await import('/src/shared/lib/back-stack.ts'); return closeTopLayer()`)
  else await press(page, 'Escape')
  await page.sleep(500)
}

async function runPair(page, entry, touch) {
  const problems = []
  entry = { ...entry, child: (touch && entry.phoneChild) || entry.child }
  const gone = (selector) => waitFor(page, selector, true)
  const opened = await openPair(page, entry, touch)
  if (opened) return [opened]

  // 1. The child shows over the parent.
  if (!(await openChild(page, entry, touch))) return ['the child did not open']
  if (!(await page.eval(ON_TOP(entry.child)))) problems.push('the child is under the parent')
  // NESTED_SHOTS=<dir>: a picture of the pair for looking at (before / after a change).
  if (process.env.NESTED_SHOTS) await page.shot(`${process.env.NESTED_SHOTS}/${entry.id}-${touch ? '390' : '1280'}.png`)
  const box = await page.eval(LAYER(entry.child))
  const view = await page.eval(`return [innerWidth, innerHeight]`)
  if (box && (box[0] < -1 || box[1] < -1 || box[0] + box[2] > view[0] + 1 || box[1] + box[3] > view[1] + 1)) problems.push(`the child is cut by the window edge: ${box.join(',')} in ${view.join('x')}`)

  // 2. A press inside the child keeps the parent.
  const inside = entry.inside
    ? await pointOf(page, entry.inside)
    : touch === false && entry.corner
      ? await page.eval(`${FIND(entry.child)} if (!layer) return null; const r = layer.getBoundingClientRect(); return [Math.round(r.left + 2), Math.round(r.top + 2)]`)
      : await page.eval(SPOT(entry.child))
  if (!inside) problems.push('nothing to press inside the child')
  else {
    await press_(page, touch, inside, 'inside')
    if (!(await page.eval(LAYER(entry.parent)))) return [...problems, 'a press inside the child closed the parent']
    const childOpen = Boolean(await page.eval(LAYER(entry.child)))
    if (entry.insideCloses ? childOpen : !childOpen) problems.push(entry.insideCloses ? 'the child stays after its choice' : 'a press inside the child closed it')
  }

  // 3. A press on the parent, outside the child, closes only the child.
  if (!(await page.eval(LAYER(entry.child)))) await openChild(page, entry, touch, true)
  // A popover that was just reopened can still be settling at its spot (it shifts to stay on screen).
  await page.sleep(500)
  const outside = await page.eval(SPOT(entry.parent, entry.child))
  if (!outside) problems.push('nothing to press on the parent')
  else {
    await press_(page, touch, outside, 'outside')
    if (!(await gone(entry.child))) problems.push('a press on the parent leaves the child open')
    if (!(await page.eval(LAYER(entry.parent)))) return [...problems, 'a press on the parent beside the child closed the parent too']
  }

  // 4. Esc / Back: one layer at a time, the top first; the focus returns to the parent.
  if (!(await page.eval(LAYER(entry.child))) && !(await openChild(page, entry, touch, true))) return [...problems, 'the child did not reopen']
  await closeTop(page, touch)
  if (!(await gone(entry.child))) problems.push(touch ? 'Back does not close the child' : 'Esc does not close the child')
  await detached(page, entry.child)
  await page.sleep(250)
  if (!(await page.eval(LAYER(entry.parent)))) return [...problems, touch ? 'Back closed the parent with the child' : 'Esc closed the parent with the child']
  if (!(await page.eval(FOCUS_IN(entry.parent)))) {
    const where = await page.eval(`const a = document.activeElement; return a ? a.tagName.toLowerCase() + (a.getAttribute('data-slot') ? '[' + a.getAttribute('data-slot') + ']' : '') : 'nothing'`)
    problems.push(`the focus did not return to the parent (it is on ${where})`)
  }
  if (!(await closeLayer(page, entry.parent, touch))) problems.push('the parent does not close with the second Esc / Back')
  await detached(page, entry.parent)
  await page.sleep(300)
  // Nothing of the layers is left behind: the page takes presses and scrolls again.
  const left = await page.eval(`
    const style = getComputedStyle(document.body)
    return { pointer: style.pointerEvents, locked: document.body.hasAttribute('data-scroll-locked'), layers: [...document.querySelectorAll('[data-dismissable-layer]:not([data-slot="tooltip-content"])')].map((n) => (n.getAttribute('data-slot') ?? n.tagName) + ':' + (n.getAttribute('data-overlay') ?? '') + ':' + n.getAttribute('data-state')).join(',') }`)
  if (left.pointer === 'none') problems.push('the page stays unpressable after both layers closed')
  if (left.locked) problems.push('the page stays scroll-locked after both layers closed')
  if (left.layers) problems.push(`a layer stays in the page after both closed: ${left.layers}`)
  return problems
}

/** All pairs at the current window size; `only` filters by id. */
export async function nestedLayers(page, { touch, label, only, shot }) {
  const problems = []
  for (const entry of NESTED) {
    if (only?.length && !only.includes(entry.id)) continue
    if (touch && entry.desktop) continue
    try {
      for (const problem of await runPair(page, entry, touch)) problems.push(`${label} nested ${entry.id}: ${problem}`)
    } catch (error) {
      problems.push(`${label} nested ${entry.id}: ${String(error.message ?? error).split('\n')[0]}`)
    }
    await press(page, 'Escape')
    await press(page, 'Escape')
    await page.sleep(200)
  }
  return problems
}
