// What check-layout visits and what it measures on each view.
import { pointOf, press } from './overlay-steps.mjs'
import { DIALOG_RECT, INSTALL_OBSERVERS, NARROW_FIELDS, OVERFLOW, READ_OBSERVERS, RESET_OBSERVERS, ROW_RECTS, TOUCH } from './probes.mjs'

export const DESKTOP = [
  [1920, 1080],
  [1440, 900],
  [1280, 720],
  [1024, 640],
  [768, 800],
  [720, 620],
  // A tiled window on Hyprland/Sway: a mouse in the phone layout (tiling ignores minWidth).
  [560, 860],
]
export const PHONE = [
  [390, 844],
  [360, 740],
]

/** Every view in full at these sizes; the rest get the main screens only. */
const FULL_AT = new Set(['1280x720', '1024x640', '768x800', '560x860', '390x844', '360x740'])

const PHONE_AT = new Set(['390x844', '360x740'])

const MAIN_ROUTES = ['/', '/stores', '/transactions', '/money/where', '/automations/reminders', 'settings:app:app', 'settings:accounting:currencies']

const STATIC_ROUTES = [
  '/',
  '/stores',
  '/transactions',
  '/transactions/contacts',
  '/transactions/calendar',
  '/transactions/bonuses',
  '/money',
  '/money/where',
  '/money/wealth',
  '/money/investments',
  '/money/month',
  '/money/health',
  '/automations',
  '/money/calculator',
  '/money/plan',
  '/automations/reminders',
  '/automations/alerts',
  '/automations/workflows',
  '/automations/journal',
]

const APP = `const app = document.querySelector('#app').__vue_app__; const gp = app.config.globalProperties;`

/** Close what pops up by itself (What's new, tours) so it does not cover the view. */
const DISMISS = `
  document.querySelector('.driver-popover-close-btn')?.click()
  for (const button of document.querySelectorAll('[role="dialog"] button')) {
    if (/^(Понятно|Got it)$/.test(button.textContent.trim())) button.click()
  }
  return true
`

/**
 * Log in through the stores. On a cold start Vite pre-bundles dependencies and reloads
 * the page meanwhile, so wait for the app to mount and try again if a reload cut in.
 */
export async function login(page) {
  for (let attempt = 1; ; attempt++) {
    try {
      await loginOnce(page)
      return
    } catch (error) {
      if (attempt === 3) throw error
      await page.sleep(2000)
    }
  }
}

/** Wait for the app to mount (a cold Vite start may reload the page meanwhile). */
export async function mounted(page) {
  await page.eval(`
    for (let tries = 0; tries < 300; tries++) {
      if (document.querySelector('#app')?.__vue_app__) return true
      await new Promise((done) => setTimeout(done, 100))
    }
    throw new Error('the app did not mount in 30 s')`)
}

async function loginOnce(page) {
  await mounted(page)
  await page.eval(INSTALL_OBSERVERS)
  await page.eval(`${APP}
    // Tours and What's new pop over the views: mark them all seen first.
    const { TOURS } = await import('/src/shared/lib/tour/tours.ts')
    const { WHATS_NEW } = await import('/src/shared/config/whats-new.ts')
    const settings = gp.$pinia._s.get('settings')
    await settings.loadApp()
    await settings.updateApp({ completed_tours: Object.keys(TOURS), seen_whats_new: WHATS_NEW.map((entry) => entry.id) })
    await gp.$pinia._s.get('vault').unlock('/tmp/mock.wallet', 'secret')
    // Russian labels are the longer ones: overflow shows up there first.
    const { setLocalePreference } = await import('/src/shared/lib/locale.ts')
    await setLocalePreference('ru')
    await gp.$router.push('/')
    return true`)
  await page.sleep(1500)
  await page.eval(DISMISS)
  await page.sleep(400)
  // Answers come a little later than the route's first frame, as from the real backend.
  await page.eval(`window.__mockLatency = 150; return true`)
}

export async function setTheme(page, theme) {
  await page.eval(`${APP} await gp.$pinia._s.get('settings').updateApp({ theme: '${theme}' }); return true`)
  await page.sleep(300)
}

/** The views: static routes, wallets of each kind, their settings, every settings tab. */
export async function views(page, size) {
  const stores = await page.eval(`
    const { call } = await import('/src/shared/api/ipc.ts')
    const tree = await call('get_store_tree')
    const seen = new Set()
    const out = []
    for (const node of tree) {
      const kind = node.store?.kind ?? node.kind
      if (seen.has(kind)) continue
      seen.add(kind)
      out.push(node.store?.id ?? node.id)
    }
    return out`)
  const tabs = await page.eval(`
    const { SETTINGS_GROUPS } = await import('/src/widgets/settings-shell/settings-shell.constants.ts')
    return SETTINGS_GROUPS.flatMap((group) => group.tabs.map((tab) => group.id + ':' + tab.sections[0].id))`)
  // The plan with its goal and limit lists unfolded (a link's `?focus=`): wide tables and cards live there.
  const goalId = PHONE_AT.has(size)
    ? await page.eval(`
        const { call } = await import('/src/shared/api/ipc.ts')
        return (await call('list_goals'))[0]?.id ?? null`)
    : null
  const all = [
    ...(goalId ? [`/money/plan?focus=${goalId}`] : []),
    ...STATIC_ROUTES,
    ...stores.flatMap((id) => [`/stores/${id}`, `/stores/${id}/settings`]),
    ...tabs.map((id) => `settings:${id}`),
  ]
  return FULL_AT.has(size) ? all : all.filter((view) => MAIN_ROUTES.includes(view))
}

async function openOnce(page, view) {
  const target = view.startsWith('settings:')
    ? `{ path: '/', query: { settings: '${view.split(':')[1]}', section: '${view.split(':')[2]}' } }`
    : `'${view}'`
  // The new route's own first frames are what the click asked for: a browser leaves shifts
  // within 500 ms of input out of CLS. Counted is what moves after that — data landing.
  await page.eval(`${APP}
    await gp.$router.push(${target})
    await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)))
    return true`)
  await page.eval(RESET_OBSERVERS)
  await page.sleep(700)
  await page.eval(DISMISS)
  await page.sleep(500)
}

/**
 * Open a view. Vite reloads the page once when a route first needs a dependency it has
 * not pre-bundled (the vault locks again, the observers are gone): log in and open again.
 */
export async function open(page, view) {
  if (!(await page.eval(`return Boolean(window.__layout)`))) await login(page)
  await openOnce(page, view)
  if (await page.eval(`return Boolean(window.__layout)`)) return
  await login(page)
  await openOnce(page, view)
}

export async function measureView(page, touch) {
  const problems = [...(await page.eval(OVERFLOW)), ...(await page.eval(NARROW_FIELDS))]
  const seen = await page.eval(READ_OBSERVERS)
  if (seen.cls > 0.01) problems.push(`layout shift ${seen.cls.toFixed(3)}: ${seen.shifts.slice(0, 3).join(', ')}`)
  const long = seen.long.filter((ms) => ms > 500)
  if (long.length) problems.push(`long task ${Math.max(...long)} ms`)
  if (touch) problems.push(...(await page.eval(TOUCH)))
  else problems.push(...(await hoverShift(page)))
  return problems
}

/** Hover a row, then leave: nothing in it (or around it) may move. */
async function hoverShift(page) {
  const before = await page.eval(ROW_RECTS)
  if (!before.length) return []
  // Start from outside: a pointer that never moved does not enter anything.
  await page.mouse(2, 2)
  await page.sleep(50)
  const problems = []
  for (const [index, row] of before.entries()) {
    await page.mouse(row.x, row.y)
    await page.sleep(250)
    const during = await page.eval(ROW_RECTS)
    await page.mouse(2, 2)
    await page.sleep(250)
    const after = await page.eval(ROW_RECTS)
    const delta = (other) =>
      Math.max(0, ...row.parts.flatMap((rect, at) => rect.map((value, axis) => Math.abs(value - (other[index]?.parts[at]?.[axis] ?? value)))))
    const moved = Math.max(delta(during), delta(after))
    if (moved > 0) problems.push(`row ${index + 1} moves ${moved}px on hover/leave`)
  }
  return problems
}

/** A dialog must open where it ends up: no slide from an offset. */
export async function dialogJump(page) {
  await page.eval(`${APP} await gp.$router.push('/'); return true`)
  await page.sleep(500)
  const opened = await page.eval(`
    const button = document.querySelector('[data-tour="quick-add"]')
    if (!button) return false
    button.click()
    return true`)
  if (!opened) return ['quick add button ([data-tour="quick-add"]) not found']
  const samples = []
  // Right after the click, mid-animation, and once it has settled.
  for (const wait of [0, 40, 40, 60, 460]) {
    if (wait) await page.sleep(wait)
    samples.push(await page.eval(DIALOG_RECT))
  }
  await page.eval(`document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); return true`)
  await page.sleep(300)
  const final = samples.at(-1)
  if (!final) return ['quick add dialog did not open']
  // The centre, not the corner: a scale-in grows the box around a fixed centre.
  const centre = (rect) => [rect[0] + rect[2] / 2, rect[1] + rect[3] / 2]
  const [finalX, finalY] = centre(final)
  const worst = Math.round(
    Math.max(...samples.filter(Boolean).map((rect) => {
      const [x, y] = centre(rect)
      return Math.max(Math.abs(x - finalX), Math.abs(y - finalY))
    })),
  )
  return worst > 2 ? [`dialog moves ${worst}px while it opens`] : []
}

/** Visiting a screen again must not add backend event listeners. */
export async function listenerLeak(page) {
  const count = `const { mockListenerCount } = await import('/src/shared/api/mock-bus.ts'); return mockListenerCount('prices-updated')`
  await page.eval(`${APP} await gp.$router.push('/transactions'); return true`)
  await page.sleep(500)
  const first = await page.eval(count)
  for (let round = 0; round < 3; round++) {
    await page.eval(`${APP} await gp.$router.push('/'); return true`)
    await page.sleep(300)
    await page.eval(`${APP} await gp.$router.push('/transactions'); return true`)
    await page.sleep(300)
  }
  const last = await page.eval(count)
  return last > first ? [`'prices-updated' listeners grow per visit: ${first} → ${last}`] : []
}

/** Back closes the top layer for settings and the quick-add dialog. */
export async function backCloses(page) {
  const problems = []
  const back = `const { closeTopLayer } = await import('/src/shared/lib/back-stack.ts'); return closeTopLayer()`
  await page.eval(`${APP} await gp.$router.push({ path: '/', query: { settings: 'app' } }); return true`)
  await page.sleep(600)
  await page.eval(back)
  await page.sleep(400)
  if (await page.eval(`${APP} return Boolean(gp.$route.query.settings)`)) problems.push('Back does not close the settings window')
  for (const [name, opener] of [
    ['quick add', `document.querySelector('[data-tour="quick-add"]')?.click()`],
    ['command palette', `document.querySelector('[data-command-palette-trigger]')?.click()`],
  ]) {
    await page.eval(`${opener}; return true`)
    await page.sleep(400)
    const open = `return Boolean(document.querySelector('[role="dialog"][data-state="open"], [data-command-palette][data-open="true"]'))`
    if (!(await page.eval(open))) {
      problems.push(`${name} did not open (no trigger found)`)
      continue
    }
    await page.eval(back)
    await page.sleep(400)
    if (await page.eval(open)) problems.push(`Back does not close the ${name}`)
  }
  return problems
}

/** A confirm button opens its question beside itself: the button keeps its box and its place, the layer does not cover it, Esc closes it. */
export async function confirmStable(page) {
  await page.eval(`${APP} await gp.$router.push('/automations/reminders'); return true`)
  await page.sleep(700)
  const at = await pointOf(page, '[data-slot="confirm-button"]')
  if (!at) return ['no [data-slot="confirm-button"] on /automations/reminders']
  const box = `const b = document.querySelector('[data-slot="confirm-button"][data-armed]') ?? [...document.querySelectorAll('[data-slot="confirm-button"]')].find((el) => el.getBoundingClientRect().width); const r = b.getBoundingClientRect(); return [r.left, r.top, r.width, r.height]`
  const before = await page.eval(box)
  await page.mouse(at[0], at[1])
  await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: at[0], y: at[1], button: 'left', clickCount: 1 })
  await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: at[0], y: at[1], button: 'left', clickCount: 1 })
  await page.sleep(500)
  const layer = await page.eval(`
    const el = document.querySelector('[data-overlay="confirm"]')
    if (!el) return null
    const r = el.getBoundingClientRect()
    return { box: [r.left, r.top, r.right, r.bottom], armed: Boolean(document.querySelector('[data-slot="confirm-button"][data-armed]')) }`)
  if (!layer) return ['the confirm button did not open its question']
  const problems = []
  const after = await page.eval(box)
  if (before.some((v, i) => Math.abs(v - after[i]) > 0.5)) problems.push(`confirm button moves or resizes when armed: ${before} → ${after}`)
  if (!layer.armed) problems.push('confirm button has no data-armed while its question is open')
  const [bl, bt, bw, bh] = after
  const [l, t, r, b] = layer.box
  const phone = r - l > 600 || l <= 0
  if (!phone && l < bl + bw && r > bl && t < bt + bh && b > bt) problems.push('the question covers the button it belongs to')
  await press(page, 'Escape')
  await page.sleep(500)
  if (await page.eval(`return Boolean(document.querySelector('[data-overlay="confirm"]'))`)) problems.push('Esc does not close the confirm question')
  return problems
}

/** A toast over an open dialog can be clicked, and the click does not close the dialog. */
export async function toastOverDialog(page) {
  await page.eval(`${APP} await gp.$router.push({ path: '/', query: { settings: 'app' } }); return true`)
  await page.sleep(600)
  await page.eval(`
    const { notify } = await import('/src/shared/lib/notify/index.ts')
    window.__undo = 0
    notify.success('check-layout', { action: { label: 'Undo', onClick: () => window.__undo++ } })
    return true`)
  await page.sleep(500)
  const at = await page.eval(`
    const button = [...document.querySelectorAll('[data-sonner-toast] button')].find((el) => el.textContent.trim() === 'Undo')
    if (!button) return null
    const rect = button.getBoundingClientRect()
    return [Math.round(rect.left + rect.width / 2), Math.round(rect.top + rect.height / 2)]`)
  if (!at) return ['toast with an action did not show']
  await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: at[0], y: at[1], button: 'left', clickCount: 1 })
  await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: at[0], y: at[1], button: 'left', clickCount: 1 })
  await page.sleep(400)
  const result = await page.eval(`${APP} return { undo: window.__undo, open: Boolean(gp.$route.query.settings) }`)
  const problems = []
  if (!result.undo) problems.push('the toast action cannot be clicked while a dialog is open')
  if (!result.open) problems.push('clicking a toast closes the dialog under it')
  await page.eval(`${APP} await gp.$router.push('/'); return true`)
  return problems
}
